fn main() {
    // Path is relative to the package root: rebuild when the migrations
    // directory changes so `sqlx::migrate!()` in
    // `src/infrastructure/repositories/postgres.rs` re-embeds them as
    // `MIGRATOR`.
    println!("cargo:rerun-if-changed=migrations");
}
