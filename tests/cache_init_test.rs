use clack::cache::db::{create_cache_pool, get_cache_db_path};
use std::fs;

#[test]
fn test_cache_db_initialization() {
    // Clean up any existing test database
    if let Ok(db_path) = get_cache_db_path() {
        let _ = fs::remove_file(&db_path);
    }

    // Initialize the cache
    let result = create_cache_pool(true);
    assert!(result.is_ok(), "Failed to create cache pool: {:?}", result);

    // Verify database file was created
    let db_path = get_cache_db_path().expect("Failed to get cache db path");
    assert!(db_path.exists(), "Database file was not created at {:?}", db_path);

    println!("✓ Database created at: {:?}", db_path);
}
