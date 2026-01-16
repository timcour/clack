# Async Cache Operations

**Status**: Open
**Priority**: Medium
**Type**: Enhancement
**Created**: 2026-01-15

## Problem

During Phase 3 implementation of the object caching feature, we initially attempted to use `diesel-async` for fully asynchronous database operations. However, we encountered significant compatibility issues with SQLite's async implementation that forced us to fall back to synchronous diesel operations.

## What We Ran Into

### 1. AsyncConnectionWrapper Complexity

The `diesel-async` crate's SQLite support uses `AsyncConnectionWrapper<C, B>` which wraps synchronous diesel connections. This wrapper has complex type parameter requirements:

```rust
// Attempted type definition that failed
pub type AsyncSqliteConnection = AsyncConnectionWrapper<SqliteConnection, diesel::sqlite::Sqlite>;
```

**Issues encountered:**
- Requires both connection type `C` and backend type `B` as generic parameters
- Not compatible with traditional connection pooling via `deadpool`
- The wrapper doesn't implement `PoolableConnection` trait needed for pooling

### 2. Connection Pooling Incompatibility

Standard async connection pooling with `deadpool` expects connections that implement `PoolableConnection`:

```rust
// This pattern doesn't work with AsyncConnectionWrapper
pub type CachePool = AsyncPool<AsyncConnectionWrapper<SqliteConnection, Sqlite>>;
pub type CacheConnection = Object<AsyncDieselConnectionManager<AsyncConnectionWrapper<...>>>;
```

**Error received:**
```
error[E0277]: the trait bound `AsyncConnectionWrapper<SqliteConnection, Sqlite>: PoolableConnection` is not satisfied
```

The async wrapper doesn't satisfy the trait bounds required for pooling because:
- It doesn't implement `DerefMut`
- It doesn't implement `PoolableConnection`
- It requires special handling for wrapping sync connections

### 3. diesel-async Feature Flags

The `diesel-async` crate has different backend features:
- `sqlite` feature doesn't exist - causes cargo errors
- `async-connection-wrapper` is the correct feature for SQLite
- This feature uses a different architecture than PostgreSQL/MySQL async backends

## Current Synchronous Implementation

We fell back to synchronous diesel with the following approach:

```rust
// Simple types without async complexity
pub type CachePool = Arc<Mutex<String>>; // Just stores DB path
pub type CacheConnection = SqliteConnection; // Synchronous connection

// Connection creation on-demand
pub async fn get_connection(pool: &CachePool) -> Result<CacheConnection> {
    let db_url = pool.lock().await.clone();
    let conn = SqliteConnection::establish(&db_url)?;
    Ok(conn)
}

// Synchronous operations
pub fn get_user(conn: &mut CacheConnection, ...) -> Result<Option<User>> {
    // Direct diesel queries without .await
    users.filter(...).first(conn).optional()?
}
```

## Downsides of Synchronous Approach

### 1. **Thread Blocking**
- Synchronous diesel operations block the tokio worker thread
- SQLite file I/O can cause brief pauses in async runtime
- Not ideal for high-concurrency scenarios

### 2. **No Parallelism for Cache Operations**
- Cache operations must be fully sequential
- Cannot leverage async/await for concurrent cache reads
- May impact performance when caching many objects simultaneously

### 3. **Mixed Async/Sync Codebase**
- Main application uses async/await (tokio runtime)
- Cache layer uses synchronous operations
- Inconsistent patterns may confuse future contributors

### 4. **Resource Efficiency**
- Each connection blocks a thread during I/O
- True async I/O would allow thread to handle other tasks
- Connection pooling would be more efficient with proper async

### 5. **Scalability Concerns**
- Under high load, blocking operations can saturate thread pool
- May need to increase tokio worker threads to compensate
- True async would scale better with many concurrent requests

## Why It's Acceptable For Now

Despite the downsides, the synchronous approach is reasonable for current use case:

1. **SQLite is file-based** - most operations are very fast (microseconds)
2. **Local cache** - no network latency involved
3. **WAL mode enabled** - allows concurrent reads without blocking
4. **Expected usage** - CLI tool with relatively low concurrency needs
5. **Small dataset** - caching workspace metadata, not millions of records

## Future Async Implementation Approach

### Option 1: Custom Async Wrapper (Recommended)

Use `tokio::task::spawn_blocking` to run sync diesel operations on a dedicated thread pool:

```rust
use tokio::task;

pub struct AsyncCachePool {
    db_path: String,
    // Could add an actual sync connection pool here
}

impl AsyncCachePool {
    pub async fn get_user(&self, workspace_id: &str, user_id: &str) -> Result<Option<User>> {
        let db_path = self.db_path.clone();
        let workspace_id = workspace_id.to_string();
        let user_id = user_id.to_string();

        task::spawn_blocking(move || {
            let mut conn = SqliteConnection::establish(&db_path)?;
            cache::operations::get_user(&mut conn, &workspace_id, &user_id, false)
        })
        .await
        .context("Task panicked")??
    }
}
```

**Benefits:**
- True async without blocking tokio workers
- Keeps existing diesel code working
- Can use `deadpool` with sync connections in the blocking pool
- Simple to implement incrementally

**Drawbacks:**
- Thread context switching overhead
- More complex error handling (unwrap spawn result)

### Option 2: Wait for diesel-async SQLite Improvements

Monitor diesel-async for better SQLite support:

```rust
// Future ideal implementation if diesel-async improves
pub type CachePool = AsyncPool<AsyncSqliteConnection>;

pub async fn get_user(pool: &CachePool, ...) -> Result<Option<User>> {
    let mut conn = pool.get().await?;
    users.filter(...).first(&mut conn).await.optional()
}
```

**Watch for:**
- New SQLite async backend (not wrapper-based)
- Better connection pooling support
- Simplified type system for SQLite async

**Timeline:** Unknown - may be several releases away

### Option 3: Switch to PostgreSQL

For production deployments requiring high concurrency:

- PostgreSQL has full `diesel-async` support with `bb8` or `deadpool`
- True async operations without wrappers
- Better concurrent access than SQLite

```rust
// PostgreSQL has first-class async support
pub type CachePool = AsyncPool<AsyncPgConnection>;
pub type CacheConnection = Object<AsyncDieselConnectionManager<AsyncPgConnection>>;

// Works seamlessly
let mut conn = pool.get().await?;
users.filter(...).first(&mut conn).await
```

**Considerations:**
- Requires running PostgreSQL server
- More operational complexity
- Overkill for CLI tool use case
- Better suited for server/daemon mode

## Recommended Next Steps

1. **Short term (current)**: Ship with synchronous implementation
   - Performance is acceptable for CLI use case
   - Simpler code is easier to maintain
   - Focus on completing core features first

2. **Medium term (future optimization)**: Implement Option 1 if needed
   - Profile actual performance under realistic workloads
   - If blocking is measurable issue, add `spawn_blocking` wrapper
   - Can be done incrementally without major refactoring

3. **Long term (architectural)**: Consider based on product direction
   - If building server/daemon: evaluate PostgreSQL migration
   - If CLI remains primary use case: current approach is fine
   - Monitor diesel-async for SQLite improvements

## Implementation Checklist

When implementing async cache operations in the future:

- [ ] Create benchmark suite to measure current sync performance baseline
- [ ] Implement `spawn_blocking` wrapper for cache operations
- [ ] Add actual connection pooling for sync connections (e.g., `r2d2`)
- [ ] Measure performance improvement and thread utilization
- [ ] Update all cache call sites to use async operations
- [ ] Add integration tests for concurrent cache access
- [ ] Document async patterns in codebase

## References

- **diesel-async docs**: https://docs.rs/diesel-async/latest/diesel_async/
- **AsyncConnectionWrapper source**: https://github.com/weiznich/diesel_async/blob/main/src/async_connection_wrapper.rs
- **tokio spawn_blocking**: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
- **Related issue tracker**: [Link to GitHub issue when created]

## Related Code

- `src/cache/db.rs` - Connection management
- `src/cache/operations.rs` - All cache operations (currently sync)
- Phase 3 implementation in `thoughts/shared/plans/2026-01-15-object-caching.md`
