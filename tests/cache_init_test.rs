use clack::cache::db::init_cache_db_at_path;
use tempfile::tempdir;

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

    // Verify WAL files were created
    let wal_path = temp_dir.path().join("test_cache.db-wal");
    let shm_path = temp_dir.path().join("test_cache.db-shm");
    assert!(wal_path.exists(), "WAL file was not created");
    assert!(shm_path.exists(), "SHM file was not created");

    println!("✓ Database created at: {:?}", db_path);
    println!("✓ WAL mode enabled (wal and shm files present)");

    // temp_dir will be automatically cleaned up when it goes out of scope
}
