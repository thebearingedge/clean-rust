//! Integration tests for the [`PgTodosUnitOfWork`] adapter against a real
//! Postgres database.

use sqlx::PgPool;

use database::{Claim, Database, IdempotencyKey, IdempotencyRepository, PgIdempotencyRepository};
use todos::models::todo::NewTodoModel;
use todos::{PgTodosRepository, PgTodosUnitOfWork, TodosRepository, TodosUnitOfWork};

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

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn commit_persists_the_todo(postgres: PgPool) {
    let unit_of_work = PgTodosUnitOfWork::new(Database::Pool(postgres.clone()));
    let new = new_todo("committed through the unit");

    let todo = {
        let unit = unit_of_work.begin().await.unwrap();
        let todo = unit.todos().add(&new).await.unwrap();
        unit.commit().await.unwrap();
        todo
    };

    let repository = PgTodosRepository::new(Database::Pool(postgres));
    let stored = repository.get_by_id(todo.todo_id).await.unwrap();
    assert_eq!(stored.map(|t| t.task), Some(new.task));
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn dropping_the_unit_rolls_back_the_todo(postgres: PgPool) {
    let unit_of_work = PgTodosUnitOfWork::new(Database::Pool(postgres.clone()));
    let new = new_todo("rolled back with the unit");

    let unit = unit_of_work.begin().await.unwrap();
    let todo = unit.todos().add(&new).await.unwrap();
    drop(unit);

    let stored = PgTodosRepository::new(Database::Pool(postgres))
        .get_by_id(todo.todo_id)
        .await
        .unwrap();
    assert!(stored.is_none());
}

#[sqlx::test(migrator = "todos::MIGRATOR")]
async fn unit_reads_and_writes_share_one_transaction(postgres: PgPool) {
    let unit_of_work = PgTodosUnitOfWork::new(Database::Pool(postgres.clone()));
    let new = new_todo("shared with the key");
    let key = key("unit-shared-key");

    let todo = {
        let unit = unit_of_work.begin().await.unwrap();
        let claim = unit
            .idempotency()
            .claim(&key, "fingerprint-1")
            .await
            .unwrap();
        assert_eq!(claim, Claim::Fresh);

        let todo = unit.todos().add(&new).await.unwrap();
        unit.idempotency().record(&key, todo.todo_id).await.unwrap();
        unit.commit().await.unwrap();
        todo
    };

    let claim = PgIdempotencyRepository::new(Database::Pool(postgres))
        .claim(&key, "fingerprint-1")
        .await
        .unwrap();
    assert_eq!(
        claim,
        Claim::Replay {
            entity_id: todo.todo_id
        }
    );
}
