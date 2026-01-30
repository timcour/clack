pub mod search;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Set up a signal handler for graceful shutdown on Ctrl+C
pub fn setup_signal_handler() -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        eprintln!("\nStopping stream...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    running
}

/// State for tracking seen messages and poll timing
pub struct StreamState {
    /// Set of seen message keys (channel_id, ts) to avoid duplicates
    seen_messages: HashSet<(String, String)>,

    /// Last poll timestamp
    last_poll: Instant,

    /// Poll interval
    interval: Duration,
}

impl StreamState {
    pub fn new(interval_secs: u64) -> Self {
        Self {
            seen_messages: HashSet::new(),
            last_poll: Instant::now(),
            interval: Duration::from_secs(interval_secs),
        }
    }

    /// Returns true if this message is new (not seen before)
    /// Adds the message to the seen set
    pub fn is_new(&mut self, channel_id: &str, ts: &str) -> bool {
        self.seen_messages
            .insert((channel_id.to_string(), ts.to_string()))
    }

    /// Wait for next poll interval
    pub async fn wait_for_next_poll(&mut self) {
        let elapsed = self.last_poll.elapsed();
        if elapsed < self.interval {
            tokio::time::sleep(self.interval - elapsed).await;
        }
        self.last_poll = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_state_is_new() {
        let mut state = StreamState::new(10);

        // First time seeing a message - should be new
        assert!(state.is_new("C123", "1234567890.123456"));

        // Second time - should not be new
        assert!(!state.is_new("C123", "1234567890.123456"));

        // Different message - should be new
        assert!(state.is_new("C123", "1234567890.123457"));

        // Same ts, different channel - should be new
        assert!(state.is_new("C456", "1234567890.123456"));
    }
}
