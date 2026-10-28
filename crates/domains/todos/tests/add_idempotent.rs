//! Integration tests for
//! [`TodosInteractor::add_idempotent`](todos::TodosInteractor) against a real
//! Postgres database.

use std::sync::Arc;

use sqlx::PgPool;

use database::{Claim, Database, IdempotencyKey, IdempotencyRepository, PgIdempotencyRepository};
use todos::models::todo::NewTodoModel;
use todos::{PgTodosRepository, PgTodosUnitOfWork, TodoError, TodosInteractor};

#[must_use]
fn new_todo(task: &str) -> NewTodoModel {
    NewTodoModel {
        task: task.to_string(),
        completed_at: None,
    }
}

#[must_use]
fn key(raw: &str) -> IdempotencyKey {
    IdempotencyKey::parse(raw).expect("test key must be valid")
}

#[must_use]
fn interactor(postgres: &PgPool) -> TodosInteractor {
    TodosInteractor::new(
        Arc::new(PgTodosRepository::new(Database::Pool(postgres.clone()))),
        Arc::new(PgTodosUnitOfWork::new(Database::Pool(postgres.clone()))),
    )
}

async fn todo_row_count(postgres: &PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from todos")
        .fetch_one(postgres)
        .await
        .unwrap()
}

async fn completed_key_count(postgres: &PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from idempotency_keys where status = 'completed'")
        .fetch_one(postgres)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn fresh_key_creates_the_todo(postgres: PgPool) {
    let interactor = interactor(&postgres);
    let new = new_todo("created once");

    let creation = interactor
        .add_idempotent(&key("fresh"), &new)
        .await
        .unwrap();

    assert!(!creation.replayed);
    assert_eq!(creation.todo.task, new.task);
    assert_eq!(todo_row_count(&postgres).await, 1);
    assert_eq!(completed_key_count(&postgres).await, 1);
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn same_key_same_body_replays(postgres: PgPool) {
    let interactor = interactor(&postgres);
    let new = new_todo("created once");
    let key = key("same-body");

    let first = interactor.add_idempotent(&key, &new).await.unwrap();
    let second = interactor.add_idempotent(&key, &new).await.unwrap();

    assert!(!first.replayed);
    assert!(second.replayed);
    assert_eq!(second.todo.todo_id, first.todo.todo_id);
    assert_eq!(second.todo.task, first.todo.task);
    assert_eq!(todo_row_count(&postgres).await, 1);
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn same_key_different_body_is_reused(postgres: PgPool) {
    let interactor = interactor(&postgres);
    let key = key("different-body");

    let first = interactor
        .add_idempotent(&key, &new_todo("the original"))
        .await
        .unwrap();

    let second = interactor
        .add_idempotent(&key, &new_todo("a different task"))
        .await;

    assert!(matches!(second, Err(TodoError::IdempotencyKeyReused)));
    assert_eq!(todo_row_count(&postgres).await, 1);

    let stored = interactor.get_by_id(first.todo.todo_id).await.unwrap();
    assert_eq!(stored.task, "the original");
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn in_flight_key_is_rejected(postgres: PgPool) {
    let key = key("in-flight");

    let claim = PgIdempotencyRepository::new(Database::Pool(postgres.clone()))
        .claim(&key, &new_todo("pending elsewhere").fingerprint())
        .await
        .unwrap();
    assert_eq!(claim, Claim::Fresh);

    let outcome = interactor(&postgres)
        .add_idempotent(&key, &new_todo("pending elsewhere"))
        .await;

    assert!(matches!(outcome, Err(TodoError::IdempotencyKeyInFlight)));
    assert_eq!(todo_row_count(&postgres).await, 0);
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn invalid_task_never_touches_the_key(postgres: PgPool) {
    let interactor = interactor(&postgres);

    let outcome = interactor
        .add_idempotent(&key("untouched"), &new_todo("   "))
        .await;

    assert!(matches!(outcome, Err(TodoError::EmptyTask)));

    let claim = PgIdempotencyRepository::new(Database::Pool(postgres))
        .claim(&key("untouched"), &new_todo("   ").fingerprint())
        .await
        .unwrap();
    assert_eq!(claim, Claim::Fresh);
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn concurrent_creates_on_one_key(postgres: PgPool) {
    let interactor = interactor(&postgres);
    let new = new_todo("racing creation");
    let key = key("racing");

    let (first, second) = tokio::join!(
        interactor.add_idempotent(&key, &new),
        interactor.add_idempotent(&key, &new),
    );

    let outcomes = [first, second];
    for outcome in &outcomes {
        match outcome {
            Err(TodoError::IdempotencyKeyInFlight) | Ok(_) => {}
            Err(other) => panic!("unexpected outcome: {other:?}"),
        }
    }

    let creations: Vec<uuid::Uuid> = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            Ok(creation) if !creation.replayed => Some(creation.todo.todo_id),
            _ => None,
        })
        .collect();
    assert_eq!(
        creations.len(),
        1,
        "a second creation must not happen: {outcomes:?}",
    );

    for replayed_id in outcomes.iter().filter_map(|outcome| match outcome {
        Ok(creation) if creation.replayed => Some(creation.todo.todo_id),
        _ => None,
    }) {
        assert_eq!(replayed_id, creations[0]);
    }

    assert_eq!(todo_row_count(&postgres).await, 1);
}
