//! Integration tests for the [`PgIdempotencyRepository`] adapter against a
//! real Postgres database.

use sqlx::PgPool;
use uuid::Uuid;

use database::Database;
use todos::{Claim, IdempotencyKey, IdempotencyRepository, PgIdempotencyRepository};

#[must_use]
fn key(raw: &str) -> IdempotencyKey {
    IdempotencyKey::parse(raw).expect("test key must be valid")
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn fresh_key_claims_fresh(postgres: PgPool) {
    let repository = PgIdempotencyRepository::new(Database::Pool(postgres));
    let key = key("fresh-key");

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Fresh);

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::InFlight);
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn record_completes_the_key(postgres: PgPool) {
    // A direct sqlx insert stands in for the todo creation whose entity the
    // key will replay; the `idempotency_keys.entity_id` foreign key requires
    // the row to exist before `record` runs.
    let todo_id: Uuid =
        sqlx::query_scalar("insert into todos (task) values ($1) returning todo_id")
            .bind("recorded task")
            .fetch_one(&postgres)
            .await
            .unwrap();

    let repository = PgIdempotencyRepository::new(Database::Pool(postgres));
    let key = key("record-key");

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Fresh);

    repository.record(&key, todo_id).await.unwrap();

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Replay { entity_id: todo_id });
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn different_fingerprint_is_mismatch(postgres: PgPool) {
    let repository = PgIdempotencyRepository::new(Database::Pool(postgres));
    let key = key("mismatch-key");

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Fresh);

    let claim = repository.claim(&key, "fingerprint-2").await.unwrap();
    assert_eq!(claim, Claim::Mismatch);

    let claim = repository.claim(&key, "fingerprint-3").await.unwrap();
    assert_eq!(claim, Claim::Mismatch);
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn concurrent_claims_never_both_fresh(postgres: PgPool) {
    let repository = PgIdempotencyRepository::new(Database::Pool(postgres));
    let key = key("concurrent-key");

    let (first, second) = tokio::join!(
        repository.claim(&key, "fingerprint-1"),
        repository.claim(&key, "fingerprint-1"),
    );

    let outcomes = [first.unwrap(), second.unwrap()];
    assert!(
        matches!(outcomes[0], Claim::Fresh | Claim::InFlight)
            && matches!(outcomes[1], Claim::Fresh | Claim::InFlight),
        "unexpected outcomes: {outcomes:?}",
    );
    let fresh_count = outcomes
        .iter()
        .filter(|claim| matches!(claim, Claim::Fresh))
        .count();
    assert_eq!(fresh_count, 1, "outcomes were: {outcomes:?}");
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn rollback_of_a_claimed_unit_leaves_the_key_absent(postgres: PgPool) {
    let db = Database::Pool(postgres.clone());
    let unit = db.begin().await.unwrap();

    let unit_repository = PgIdempotencyRepository::new(unit.database());
    let key = key("rollback-key");

    let claim = unit_repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Fresh);

    drop(unit_repository);
    drop(unit);

    let repository = PgIdempotencyRepository::new(Database::Pool(postgres));
    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Fresh);
}
