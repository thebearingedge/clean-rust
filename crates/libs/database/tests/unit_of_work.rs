//! Integration tests for the [`UnitOfWork`]-backed [`Database`] variants.

use std::time::Duration;

use sqlx::PgPool;

use database::Database;

async fn create_table(pool: &PgPool) {
    sqlx::query("create table unit_of_work_test (id int)")
        .execute(pool)
        .await
        .unwrap();
}

async fn row_count(pool: &PgPool) -> i64 {
    let (count,): (i64,) = sqlx::query_as("select count(*) from unit_of_work_test")
        .fetch_one(pool)
        .await
        .unwrap();
    count
}

#[sqlx::test]
async fn commit_persists(postgres: PgPool) {
    create_table(&postgres).await;
    let db = Database::Pool(postgres.clone());
    let unit = db.begin().await.unwrap();
    let mut conn = unit.database().connect().await.unwrap();
    sqlx::query("insert into unit_of_work_test (id) values (1)")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);
    unit.commit().await.unwrap();
    assert_eq!(row_count(&postgres).await, 1);
}

#[sqlx::test]
async fn dropping_the_unit_rolls_back(postgres: PgPool) {
    create_table(&postgres).await;
    let db = Database::Pool(postgres.clone());
    let unit = db.begin().await.unwrap();
    let mut conn = unit.database().connect().await.unwrap();
    sqlx::query("insert into unit_of_work_test (id) values (1)")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);
    drop(unit);
    assert_eq!(row_count(&postgres).await, 0);
}

#[sqlx::test]
async fn connect_fails_after_commit(postgres: PgPool) {
    create_table(&postgres).await;
    let db = Database::Pool(postgres);
    let unit = db.begin().await.unwrap();
    let database = unit.database();
    unit.commit().await.unwrap();
    let error = database.connect().await.unwrap_err();
    assert!(
        error.to_string().contains("transaction already finished"),
        "unexpected error: {error}"
    );
}

#[sqlx::test]
async fn lock_wait_times_out(postgres: PgPool) {
    create_table(&postgres).await;
    let db = Database::Pool(postgres);
    let unit = db.begin_with(Duration::from_millis(50)).await.unwrap();
    let database = unit.database();
    let conn = database.connect().await.unwrap();
    let error = database.connect().await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("another repository call is holding the transaction"),
        "unexpected error: {error}"
    );
    drop(conn);
    drop(unit);
}
