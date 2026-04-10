use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStats {
    pub total_requests: u64,
    pub total_errors: u64,
    pub total_successes: u64,
    pub average_response_time_ms: f64,
    pub providers_called: std::collections::HashMap<String, u64>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct RequestRecord {
    timestamp: u64,
    provider: String,
    model: String,
    success: bool,
    response_time_ms: u64,
}

pub struct StatsCollector {
    requests: Arc<RwLock<Vec<RequestRecord>>>,
    total_requests: Arc<RwLock<u64>>,
    total_errors: Arc<RwLock<u64>>,
    total_successes: Arc<RwLock<u64>>,
}

impl StatsCollector {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(RwLock::new(Vec::new())),
            total_requests: Arc::new(RwLock::new(0)),
            total_errors: Arc::new(RwLock::new(0)),
            total_successes: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn record_request(
        &self,
        provider: String,
        model: String,
        success: bool,
        response_time_ms: u64,
    ) {
        let mut total = self.total_requests.write().await;
        *total += 1;

        if success {
            let mut successes = self.total_successes.write().await;
            *successes += 1;
        } else {
            let mut errors = self.total_errors.write().await;
            *errors += 1;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let record = RequestRecord {
            timestamp,
            provider,
            model,
            success,
            response_time_ms,
        };

        let mut requests = self.requests.write().await;
        requests.push(record);

        // Keep only last 1000 requests using drain instead of remove(0)
        if requests.len() > 1000 {
            let to_remove = requests.len() - 1000;
            let _: Vec<_> = requests.drain(0..to_remove).collect();
        }
    }

    pub async fn get_stats(&self) -> RequestStats {
        let total = *self.total_requests.read().await;
        let errors = *self.total_errors.read().await;
        let successes = *self.total_successes.read().await;

        let requests = self.requests.read().await;

        let mut providers_called: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        let mut total_time: u64 = 0;

        for req in requests.iter() {
            *providers_called.entry(req.provider.clone()).or_insert(0) += 1;
            total_time += req.response_time_ms;
        }

        let avg_time = if !requests.is_empty() {
            total_time as f64 / requests.len() as f64
        } else {
            0.0
        };

        RequestStats {
            total_requests: total,
            total_errors: errors,
            total_successes: successes,
            average_response_time_ms: avg_time,
            providers_called,
        }
    }
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

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
            .record_request("anthropic".to_string(), "claude-opus".to_string(), true, 100)
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
            .record_request("anthropic".to_string(), "claude-opus".to_string(), true, 100)
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
}
