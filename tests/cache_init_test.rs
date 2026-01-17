use clack::cache::db::init_cache_db_at_path;
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel::sqlite::SqliteConnection;
use tempfile::tempdir;

#[derive(QueryableByName)]
struct JournalMode {
    #[diesel(sql_type = Text)]
    journal_mode: String,
}

#[test]
fn test_cache_db_initialization() {
    // Create a temporary directory for this test
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_cache.db");

    // Initialize the cache at the temp path
    let result = init_cache_db_at_path(&db_path, true);
    assert!(result.is_ok(), "Failed to initialize cache: {:?}", result);

    // Verify database file was created
    assert!(db_path.exists(), "Database file was not created at {:?}", db_path);

    let db_url = format!("sqlite://{}", db_path.display());
    let mut conn = SqliteConnection::establish(&db_url)
        .expect("Failed to reconnect to cache database");
    let mode = diesel::sql_query("PRAGMA journal_mode")
        .get_result::<JournalMode>(&mut conn)
        .expect("Failed to read journal_mode")
        .journal_mode;
    assert_eq!(mode.to_lowercase(), "wal", "WAL mode not enabled");

    println!("✓ Database created at: {:?}", db_path);
    println!("✓ WAL mode enabled (journal_mode = wal)");

    // temp_dir will be automatically cleaned up when it goes out of scope
}
