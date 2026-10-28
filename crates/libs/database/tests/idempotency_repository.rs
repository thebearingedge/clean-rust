//! Integration tests for the [`PgIdempotencyRepository`] adapter against a
//! real Postgres database.

use sqlx::PgPool;
use uuid::Uuid;

use database::{Claim, Database, IdempotencyKey, IdempotencyRepository, PgIdempotencyRepository};

#[must_use]
fn key(raw: &str) -> IdempotencyKey {
    IdempotencyKey::parse(raw).expect("test key must be valid")
}

async fn create_schema(pool: &PgPool) {
    sqlx::query("create type idempotency_key_status as enum ('pending', 'completed')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "create table idempotency_keys (
          idempotency_key text           not null check (idempotency_key ~ '^[!-~]+$'
                                                          and length(idempotency_key) between 1 and 64),
          fingerprint     text           not null,
          status          idempotency_key_status not null,
          entity_id       uuid,
          created_at      timestamptz(6) not null default clock_timestamp(),
          primary key (idempotency_key),
          check (status = 'pending' or entity_id is not null)
        )",
    )
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn fresh_key_claims_fresh(postgres: PgPool) {
    create_schema(&postgres).await;
    let repository = PgIdempotencyRepository::new(Database::Pool(postgres));
    let key = key("fresh-key");

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Fresh);

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::InFlight);
}

#[sqlx::test]
async fn record_completes_the_key(postgres: PgPool) {
    create_schema(&postgres).await;
    let repository = PgIdempotencyRepository::new(Database::Pool(postgres));
    let key = key("record-key");
    let entity_id = Uuid::now_v7();

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Fresh);

    repository.record(&key, entity_id).await.unwrap();

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Replay { entity_id });
}

#[sqlx::test]
async fn different_fingerprint_is_mismatch(postgres: PgPool) {
    create_schema(&postgres).await;
    let repository = PgIdempotencyRepository::new(Database::Pool(postgres));
    let key = key("mismatch-key");

    let claim = repository.claim(&key, "fingerprint-1").await.unwrap();
    assert_eq!(claim, Claim::Fresh);

    let claim = repository.claim(&key, "fingerprint-2").await.unwrap();
    assert_eq!(claim, Claim::Mismatch);

    let claim = repository.claim(&key, "fingerprint-3").await.unwrap();
    assert_eq!(claim, Claim::Mismatch);
}

#[sqlx::test]
async fn concurrent_claims_never_both_fresh(postgres: PgPool) {
    create_schema(&postgres).await;
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

#[sqlx::test]
async fn rollback_of_a_claimed_unit_leaves_the_key_absent(postgres: PgPool) {
    create_schema(&postgres).await;
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
