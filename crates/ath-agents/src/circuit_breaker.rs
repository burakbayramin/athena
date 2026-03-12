//! Circuit breaker state machine for provider reliability.
//!
//! Tracks consecutive failures per provider and prevents requests when a
//! provider is consistently failing. Uses a three-state model:
//! Closed (healthy) -> Open (tripped) -> HalfOpen (probing) -> Closed.

use std::time::Duration;
use tokio::time::Instant;

/// The internal state of a circuit breaker.
#[derive(Debug)]
pub enum CircuitState {
    /// Normal operation -- requests are allowed through.
    Closed { consecutive_failures: u32 },
    /// Circuit is tripped -- requests are blocked until cooldown expires.
    Open { opened_at: Instant },
    /// One probe request is in flight to test if the provider has recovered.
    HalfOpen,
}

/// A circuit breaker that trips after consecutive failures and recovers
/// via a half-open probe after a cooldown period.
#[derive(Debug)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_threshold: u32,
    cooldown: Duration,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given failure threshold and cooldown.
    pub fn new(failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            state: CircuitState::Closed {
                consecutive_failures: 0,
            },
            failure_threshold,
            cooldown,
        }
    }

    /// Whether a request can be attempted right now.
    ///
    /// - Closed: always true
    /// - Open: true if cooldown has elapsed (transitions to HalfOpen)
    /// - HalfOpen: false (probe already in flight)
    pub fn can_attempt(&mut self) -> bool {
        match &self.state {
            CircuitState::Closed { .. } => true,
            CircuitState::Open { opened_at } => {
                if opened_at.elapsed() >= self.cooldown {
                    self.state = CircuitState::HalfOpen;
                    true // allow one probe
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => false, // probe already in flight
        }
    }

    /// Record a successful request. Resets to Closed state from any state.
    pub fn record_success(&mut self) {
        self.state = CircuitState::Closed {
            consecutive_failures: 0,
        };
    }

    /// Record a failed request.
    ///
    /// - Closed: increments failure count, trips if threshold reached
    /// - HalfOpen: reopens circuit with fresh cooldown
    /// - Open: no-op
    pub fn record_failure(&mut self) {
        match &self.state {
            CircuitState::Closed {
                consecutive_failures,
            } => {
                let new_count = consecutive_failures + 1;
                if new_count >= self.failure_threshold {
                    self.state = CircuitState::Open {
                        opened_at: Instant::now(),
                    };
                } else {
                    self.state = CircuitState::Closed {
                        consecutive_failures: new_count,
                    };
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open {
                    opened_at: Instant::now(),
                };
            }
            CircuitState::Open { .. } => {} // already open, no-op
        }
    }

    /// Whether the circuit is currently in the Open state.
    pub fn is_open(&self) -> bool {
        matches!(self.state, CircuitState::Open { .. })
    }
}

impl Default for CircuitBreaker {
    /// Default: trips after 3 consecutive failures, 30s cooldown.
    fn default() -> Self {
        Self::new(3, Duration::from_secs(30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time;

    #[test]
    fn starts_in_closed_state() {
        let cb = CircuitBreaker::default();
        assert!(!cb.is_open());
    }

    #[tokio::test(start_paused = true)]
    async fn closed_allows_attempts() {
        let mut cb = CircuitBreaker::default();
        assert!(cb.can_attempt());
    }

    #[tokio::test(start_paused = true)]
    async fn trips_after_threshold_failures() {
        let mut cb = CircuitBreaker::default();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.can_attempt(), "should still be closed after 2 failures");
        cb.record_failure();
        assert!(cb.is_open(), "should be open after 3 failures");
        assert!(!cb.can_attempt(), "should not allow attempts when open");
    }

    #[tokio::test(start_paused = true)]
    async fn success_resets_failure_count() {
        let mut cb = CircuitBreaker::default();
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        // Should be back to 0 failures
        cb.record_failure();
        cb.record_failure();
        assert!(
            cb.can_attempt(),
            "should allow after reset + 2 new failures"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn open_blocks_until_cooldown() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(10));
        cb.record_failure(); // trips immediately (threshold = 1)
        assert!(!cb.can_attempt());

        // Advance less than cooldown
        time::advance(Duration::from_secs(5)).await;
        assert!(!cb.can_attempt(), "should block before cooldown");

        // Advance past cooldown
        time::advance(Duration::from_secs(6)).await;
        assert!(
            cb.can_attempt(),
            "should allow one probe after cooldown (transitions to HalfOpen)"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn half_open_blocks_second_attempt() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(10));
        cb.record_failure();
        time::advance(Duration::from_secs(11)).await;

        assert!(cb.can_attempt(), "first attempt in HalfOpen: allowed");
        assert!(
            !cb.can_attempt(),
            "second attempt in HalfOpen: blocked (probe in flight)"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn half_open_success_closes_circuit() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(10));
        cb.record_failure();
        time::advance(Duration::from_secs(11)).await;
        cb.can_attempt(); // transitions to HalfOpen

        cb.record_success();
        assert!(!cb.is_open(), "should be closed after success in HalfOpen");
        assert!(cb.can_attempt(), "should allow attempts after closing");
    }

    #[tokio::test(start_paused = true)]
    async fn half_open_failure_reopens_circuit() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(10));
        cb.record_failure();
        time::advance(Duration::from_secs(11)).await;
        cb.can_attempt(); // transitions to HalfOpen

        cb.record_failure();
        assert!(cb.is_open(), "should reopen after failure in HalfOpen");
        assert!(!cb.can_attempt(), "should block after reopening");
    }

    #[tokio::test(start_paused = true)]
    async fn reopened_circuit_needs_fresh_cooldown() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(10));
        cb.record_failure();
        time::advance(Duration::from_secs(11)).await;
        cb.can_attempt(); // HalfOpen
        cb.record_failure(); // reopen

        // The cooldown should restart from now
        assert!(!cb.can_attempt());
        time::advance(Duration::from_secs(5)).await;
        assert!(!cb.can_attempt(), "should block before fresh cooldown");
        time::advance(Duration::from_secs(6)).await;
        assert!(cb.can_attempt(), "should allow after fresh cooldown");
    }

    #[tokio::test(start_paused = true)]
    async fn record_failure_in_open_is_noop() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(10));
        cb.record_failure(); // trips
        assert!(cb.is_open());

        // Additional failures should not change anything
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());

        // Cooldown should still work from original trip time
        time::advance(Duration::from_secs(11)).await;
        assert!(cb.can_attempt());
    }

    #[tokio::test(start_paused = true)]
    async fn record_success_from_open_closes() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(10));
        cb.record_failure(); // trips
        assert!(cb.is_open());
        cb.record_success();
        assert!(!cb.is_open());
        assert!(cb.can_attempt());
    }

    #[test]
    fn custom_threshold_and_cooldown() {
        let mut cb = CircuitBreaker::new(5, Duration::from_secs(60));
        for _ in 0..4 {
            cb.record_failure();
        }
        assert!(!cb.is_open(), "should not trip before threshold of 5");
        cb.record_failure();
        assert!(cb.is_open(), "should trip at threshold of 5");
    }

    #[test]
    fn default_values() {
        let cb = CircuitBreaker::default();
        // Default: threshold 3, cooldown 30s
        // We can only verify threshold indirectly
        assert!(!cb.is_open());
    }
}
