use anyhow::{Context, Result};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

// For SQLite, we use synchronous connections with a mutex for thread safety
// This is simpler and more appropriate for SQLite's file-based nature
pub type CachePool = Arc<Mutex<String>>; // Stores the DB path
pub type CacheConnection = SqliteConnection;

/// Get platform-specific cache directory
pub fn get_cache_dir() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .context("Failed to determine cache directory for this platform")?;

    let clack_cache = cache_dir.join("clack");

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&clack_cache)
        .context("Failed to create clack cache directory")?;

    Ok(clack_cache)
}

/// Get full path to cache database file
pub fn get_cache_db_path() -> Result<PathBuf> {
    let cache_dir = get_cache_dir()?;
    Ok(cache_dir.join("cache.db"))
}

/// Initialize the cache database and run migrations
pub fn init_cache_db(verbose: bool) -> Result<()> {
    let db_path = get_cache_db_path()?;
    init_cache_db_at_path(&db_path, verbose)
}

/// Initialize cache database at a specific path (for testing)
pub fn init_cache_db_at_path(db_path: &PathBuf, verbose: bool) -> Result<()> {
    let db_url = format!("sqlite://{}", db_path.display());

    if verbose {
        eprintln!("Initializing cache database at: {}", db_path.display());
    }

    // Create synchronous connection for migrations
    let mut conn = SqliteConnection::establish(&db_url)
        .context("Failed to connect to cache database")?;

    // Enable WAL mode (must be done outside of a transaction)
    diesel::sql_query("PRAGMA journal_mode = WAL")
        .execute(&mut conn)
        .context("Failed to enable WAL mode")?;

    // Enable foreign keys
    diesel::sql_query("PRAGMA foreign_keys = ON")
        .execute(&mut conn)
        .context("Failed to enable foreign keys")?;

    // Run pending migrations
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to run migrations: {}", e))?;

    if verbose {
        eprintln!("Cache database initialized successfully");
    }

    Ok(())
}

/// Create a connection pool for the cache database
/// For SQLite with async-connection-wrapper, this stores the DB URL
/// Actual connections are created on demand
pub async fn create_cache_pool(verbose: bool) -> Result<CachePool> {
    // Initialize database and run migrations
    init_cache_db(verbose)?;

    let db_path = get_cache_db_path()?;
    let db_url = format!("sqlite://{}", db_path.display());

    if verbose {
        eprintln!("Cache database ready at: {}", db_url);
    }

    Ok(Arc::new(Mutex::new(db_url)))
}

/// Get a connection from the pool
/// For SQLite, this creates a new synchronous connection
pub async fn get_connection(pool: &CachePool) -> Result<CacheConnection> {
    let db_url = pool.lock().await.clone();

    // Create sync connection
    let conn = SqliteConnection::establish(&db_url)
        .context("Failed to establish SQLite connection")?;

    Ok(conn)
}
