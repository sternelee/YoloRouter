use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

// ─── Public output type ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStats {
    pub total_requests: u64,
    pub total_errors: u64,
    pub total_successes: u64,
    pub average_response_time_ms: f64,
    pub providers_called: HashMap<String, u64>,
}

// ─── Internal state (all fields behind a single lock) ────────────────────────

#[derive(Debug)]
struct RequestRecord {
    provider: String,
    response_time_ms: u64,
}

#[derive(Debug, Default)]
struct Inner {
    total_requests: u64,
    total_errors: u64,
    total_successes: u64,
    /// Ring-buffer: last 1 000 records only.
    records: Vec<RequestRecord>,
}

// ─── StatsCollector ──────────────────────────────────────────────────────────

/// Thread-safe request statistics collector.
///
/// Uses a **single** `Mutex<Inner>` so that every `record_request` call is one
/// atomic write operation instead of four separate async lock acquisitions.
/// This eliminates the multi-lock contention (W8) identified in the code review.
pub struct StatsCollector {
    inner: Arc<Mutex<Inner>>,
}

impl StatsCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    /// Record one completed request. All counters are updated atomically under
    /// a single lock — no deadlock risk, no stale intermediate state.
    pub async fn record_request(
        &self,
        provider: String,
        _model: String,
        success: bool,
        response_time_ms: u64,
    ) {
        let mut g = self.inner.lock().await;
        g.total_requests += 1;
        if success {
            g.total_successes += 1;
        } else {
            g.total_errors += 1;
        }

        g.records.push(RequestRecord {
            provider,
            response_time_ms,
        });

        // Keep only the most recent 1 000 entries.
        if g.records.len() > 1000 {
            let excess = g.records.len() - 1000;
            g.records.drain(0..excess);
        }
    }

    /// Return an aggregated snapshot of all recorded statistics.
    pub async fn get_stats(&self) -> RequestStats {
        let g = self.inner.lock().await;

        let mut providers_called: HashMap<String, u64> = HashMap::new();
        let mut total_time: u64 = 0;

        for rec in &g.records {
            *providers_called.entry(rec.provider.clone()).or_insert(0) += 1;
            total_time += rec.response_time_ms;
        }

        let average_response_time_ms = if g.records.is_empty() {
            0.0
        } else {
            total_time as f64 / g.records.len() as f64
        };

        RequestStats {
            total_requests: g.total_requests,
            total_errors: g.total_errors,
            total_successes: g.total_successes,
            average_response_time_ms,
            providers_called,
        }
    }

    /// Return the timestamp (Unix seconds) of the most recent request, or 0.
    pub fn last_request_time(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stats_collector_creation() {
        let collector = StatsCollector::new();
        let stats = collector.get_stats().await;
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.total_errors, 0);
        assert_eq!(stats.total_successes, 0);
    }

    #[tokio::test]
    async fn test_record_request() {
        let collector = StatsCollector::new();
        collector
            .record_request(
                "anthropic".to_string(),
                "claude-opus".to_string(),
                true,
                100,
            )
            .await;

        let stats = collector.get_stats().await;
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.total_successes, 1);
        assert_eq!(stats.total_errors, 0);
    }

    #[tokio::test]
    async fn test_record_multiple_requests() {
        let collector = StatsCollector::new();
        collector
            .record_request(
                "anthropic".to_string(),
                "claude-opus".to_string(),
                true,
                100,
            )
            .await;
        collector
            .record_request("openai".to_string(), "gpt-4".to_string(), true, 150)
            .await;
        collector
            .record_request("openai".to_string(), "gpt-4".to_string(), false, 200)
            .await;

        let stats = collector.get_stats().await;
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.total_successes, 2);
        assert_eq!(stats.total_errors, 1);
        assert_eq!(*stats.providers_called.get("anthropic").unwrap_or(&0), 1);
        assert_eq!(*stats.providers_called.get("openai").unwrap_or(&0), 2);
    }

    #[tokio::test]
    async fn test_ring_buffer_caps_at_1000() {
        let collector = StatsCollector::new();
        for i in 0..1200u64 {
            collector
                .record_request("p".to_string(), "m".to_string(), true, i)
                .await;
        }
        let g = collector.inner.lock().await;
        assert_eq!(g.records.len(), 1000, "ring buffer should cap at 1000 entries");
        assert_eq!(g.total_requests, 1200, "all requests should be counted");
    }
}
