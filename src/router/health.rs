use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct ProviderHealthTracker {
    entries: Mutex<HashMap<String, Instant>>,
}

impl Default for ProviderHealthTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderHealthTracker {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true if provider is currently in cooldown.
    pub fn is_cooling_down(&self, provider: &str, cooldown: Duration) -> bool {
        if cooldown.is_zero() {
            return false;
        }
        let entries = self.entries.lock().unwrap();
        if let Some(&failed_at) = entries.get(provider) {
            failed_at.elapsed() < cooldown
        } else {
            false
        }
    }

    /// Returns remaining cooldown duration, or None if not cooling down.
    pub fn remaining(&self, provider: &str, cooldown: Duration) -> Option<Duration> {
        if cooldown.is_zero() {
            return None;
        }
        let entries = self.entries.lock().unwrap();
        entries.get(provider).and_then(|&failed_at| {
            let elapsed = failed_at.elapsed();
            if elapsed < cooldown {
                Some(cooldown - elapsed)
            } else {
                None
            }
        })
    }

    /// Record a failure — sets or resets the cooldown timer.
    pub fn record_failure(&self, provider: &str) {
        let mut entries = self.entries.lock().unwrap();
        entries.insert(provider.to_string(), Instant::now());
    }

    /// Record a success — clears the cooldown entry.
    pub fn record_success(&self, provider: &str) {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(provider);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_new_provider_not_cooling_down() {
        let tracker = ProviderHealthTracker::new();
        assert!(!tracker.is_cooling_down("openai", Duration::from_secs(60)));
    }

    #[test]
    fn test_record_failure_triggers_cooldown() {
        let tracker = ProviderHealthTracker::new();
        tracker.record_failure("openai");
        assert!(tracker.is_cooling_down("openai", Duration::from_secs(60)));
    }

    #[test]
    fn test_cooldown_expires() {
        let tracker = ProviderHealthTracker::new();
        tracker.record_failure("openai");
        // With a 1ms cooldown it should expire almost immediately
        sleep(Duration::from_millis(5));
        assert!(!tracker.is_cooling_down("openai", Duration::from_millis(1)));
    }

    #[test]
    fn test_record_success_clears_cooldown() {
        let tracker = ProviderHealthTracker::new();
        tracker.record_failure("openai");
        assert!(tracker.is_cooling_down("openai", Duration::from_secs(60)));
        tracker.record_success("openai");
        assert!(!tracker.is_cooling_down("openai", Duration::from_secs(60)));
    }

    #[test]
    fn test_zero_cooldown_never_blocks() {
        let tracker = ProviderHealthTracker::new();
        tracker.record_failure("openai");
        assert!(!tracker.is_cooling_down("openai", Duration::ZERO));
    }

    #[test]
    fn test_remaining_returns_some_during_cooldown() {
        let tracker = ProviderHealthTracker::new();
        tracker.record_failure("openai");
        let rem = tracker.remaining("openai", Duration::from_secs(60));
        assert!(rem.is_some());
        assert!(rem.unwrap() <= Duration::from_secs(60));
    }

    #[test]
    fn test_remaining_returns_none_after_success() {
        let tracker = ProviderHealthTracker::new();
        tracker.record_failure("openai");
        tracker.record_success("openai");
        assert!(tracker
            .remaining("openai", Duration::from_secs(60))
            .is_none());
    }

    #[test]
    fn test_independent_providers() {
        let tracker = ProviderHealthTracker::new();
        tracker.record_failure("openai");
        assert!(tracker.is_cooling_down("openai", Duration::from_secs(60)));
        assert!(!tracker.is_cooling_down("anthropic", Duration::from_secs(60)));
    }

    #[test]
    fn test_record_failure_resets_timer() {
        let tracker = ProviderHealthTracker::new();
        tracker.record_failure("openai");
        sleep(Duration::from_millis(10));
        // Reset timer — should still be cooling down
        tracker.record_failure("openai");
        assert!(tracker.is_cooling_down("openai", Duration::from_millis(20)));
    }
}
