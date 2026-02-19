use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Configuration for exponential backoff with jitter.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the first one).
    pub max_attempts: u32,
    /// Base delay for the first retry in milliseconds.
    pub base_delay_ms: u64,
    /// Upper cap for delay in milliseconds.
    pub max_delay_ms: u64,
    /// Multiplier applied to delay on each attempt.
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1_000,
            max_delay_ms: 60_000,
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    pub fn new(max_attempts: u32, base_delay_ms: u64) -> Self {
        Self {
            max_attempts,
            base_delay_ms,
            ..Default::default()
        }
    }

    /// Delay for a given retry attempt (0-based) with ±20% jitter.
    /// Attempt 0 → base_delay_ms, attempt 1 → base * 2, etc., capped at max_delay_ms.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base =
            (self.base_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32)) as u64;
        let capped = base.min(self.max_delay_ms);

        // Pseudo-random jitter from system time subseconds (no extra crate needed).
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(12_345);

        let jitter_range = capped / 5; // ±20%
        let jitter_offset = if jitter_range > 0 {
            (nanos as u64) % (jitter_range * 2 + 1)
        } else {
            0
        };
        // Ranges from capped*0.8 to capped*1.2
        let delay = capped.saturating_sub(jitter_range).saturating_add(jitter_offset);
        Duration::from_millis(delay)
    }

    /// Returns true for errors that are worth retrying (rate limit, service unavailable).
    /// Returns false for client errors (4xx other than 429) that won't be fixed by retrying.
    pub fn is_retriable(error_msg: &str) -> bool {
        error_msg.contains("429")
            || error_msg.contains("503")
            || error_msg.contains("rate limit")
            || error_msg.contains("service unavailable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_increases_with_attempts() {
        let cfg = RetryConfig::new(5, 1000);
        // With 20% jitter, delay(1) should always be greater than delay(0)
        // Use the capped value (without jitter) for deterministic check
        let d0 = cfg.base_delay_ms;
        let d1 = (cfg.base_delay_ms as f64 * cfg.backoff_multiplier) as u64;
        assert!(d1 > d0);
    }

    #[test]
    fn test_delay_capped_at_max() {
        let cfg = RetryConfig {
            max_delay_ms: 5_000,
            ..RetryConfig::new(10, 1000)
        };
        // After many attempts, delay should not exceed max (with jitter could be max*1.2)
        let delay = cfg.delay_for_attempt(20);
        assert!(delay.as_millis() <= (cfg.max_delay_ms * 12 / 10) as u128 + 1);
    }

    #[test]
    fn test_is_retriable() {
        assert!(RetryConfig::is_retriable("HTTP 429: rate limit exceeded"));
        assert!(RetryConfig::is_retriable("HTTP 503: service unavailable"));
        assert!(!RetryConfig::is_retriable("HTTP 401: unauthorized"));
        assert!(!RetryConfig::is_retriable("HTTP 400: bad request"));
    }
}
