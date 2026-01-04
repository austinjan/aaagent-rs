//! Rate limiting and retry logic for LLM API calls.
//!
//! This module provides:
//! - Automatic retry with exponential backoff
//! - Rate limit detection from API responses
//! - Configurable retry policies

use std::time::Duration;

/// Configuration for rate limiting and retry behavior.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum number of retries before giving up
    pub max_retries: u32,

    /// Initial delay before first retry
    pub initial_delay: Duration,

    /// Maximum delay between retries
    pub max_delay: Duration,

    /// Multiplier for exponential backoff (e.g., 2.0 doubles delay each retry)
    pub backoff_multiplier: f64,

    /// Whether to add jitter to delays to avoid thundering herd
    pub add_jitter: bool,

    /// Whether to respect retry-after headers from the server
    pub respect_retry_after: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            add_jitter: true,
            respect_retry_after: true,
        }
    }
}

impl RateLimitConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set maximum delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set backoff multiplier
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Enable or disable jitter
    pub fn with_jitter(mut self, enabled: bool) -> Self {
        self.add_jitter = enabled;
        self
    }

    /// Aggressive retry config for important requests
    pub fn aggressive() -> Self {
        Self {
            max_retries: 10,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(120),
            backoff_multiplier: 1.5,
            add_jitter: true,
            respect_retry_after: true,
        }
    }

    /// Conservative retry config to avoid overloading
    pub fn conservative() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            add_jitter: true,
            respect_retry_after: true,
        }
    }

    /// No retries - fail immediately
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }
}

/// Information about a rate limit error.
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    /// Suggested retry delay from the server (if provided)
    pub retry_after: Option<Duration>,

    /// The quota that was exceeded (if known)
    pub quota_metric: Option<String>,

    /// Current usage limit
    pub limit: Option<u64>,

    /// Error message from the server
    pub message: String,
}

impl RateLimitInfo {
    /// Parse rate limit info from a Gemini API error response
    pub fn from_gemini_error(error_body: &str) -> Option<Self> {
        // Try to parse retry delay from "Please retry in X.XXXs"
        let retry_after = parse_retry_delay(error_body);

        // Try to extract quota metric
        let quota_metric =
            extract_between(error_body, "\"quotaMetric\": \"", "\"").map(|s| s.to_string());

        // Try to extract limit
        let limit =
            extract_between(error_body, "\"quotaValue\": \"", "\"").and_then(|s| s.parse().ok());

        if error_body.contains("429") || error_body.contains("RESOURCE_EXHAUSTED") {
            Some(Self {
                retry_after,
                quota_metric,
                limit,
                message: error_body.to_string(),
            })
        } else {
            None
        }
    }

    /// Parse rate limit info from an OpenAI API error response
    pub fn from_openai_error(error_body: &str) -> Option<Self> {
        // OpenAI uses "Rate limit reached" or "Too Many Requests"
        if error_body.contains("rate_limit") || error_body.contains("429") {
            let retry_after = parse_retry_delay(error_body);

            Some(Self {
                retry_after,
                quota_metric: None,
                limit: None,
                message: error_body.to_string(),
            })
        } else {
            None
        }
    }

    /// Parse rate limit info from an Anthropic API error response
    pub fn from_anthropic_error(error_body: &str) -> Option<Self> {
        if error_body.contains("rate_limit") || error_body.contains("429") {
            let retry_after = parse_retry_delay(error_body);

            Some(Self {
                retry_after,
                quota_metric: None,
                limit: None,
                message: error_body.to_string(),
            })
        } else {
            None
        }
    }
}

/// Parse retry delay from various formats in error messages.
fn parse_retry_delay(text: &str) -> Option<Duration> {
    // Format: "Please retry in X.XXXs" or "retry in Xs"
    if let Some(pos) = text.find("retry in ") {
        let remaining = &text[pos + 9..];
        if let Some(end) = remaining.find('s') {
            let delay_str = &remaining[..end];
            if let Ok(secs) = delay_str.parse::<f64>() {
                return Some(Duration::from_secs_f64(secs));
            }
        }
    }

    // Format: "retryDelay\": \"Xs\""
    if let Some(delay_str) = extract_between(text, "\"retryDelay\": \"", "\"") {
        let delay_str = delay_str.trim_end_matches('s');
        if let Ok(secs) = delay_str.parse::<f64>() {
            return Some(Duration::from_secs_f64(secs));
        }
    }

    // Format: Retry-After header value (seconds)
    if let Some(delay_str) = extract_between(text, "Retry-After: ", "\n") {
        if let Ok(secs) = delay_str.trim().parse::<u64>() {
            return Some(Duration::from_secs(secs));
        }
    }

    None
}

/// Extract text between two markers.
fn extract_between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_pos = text.find(start)? + start.len();
    let remaining = &text[start_pos..];
    let end_pos = remaining.find(end)?;
    Some(&remaining[..end_pos])
}

/// State for tracking retry attempts.
#[derive(Debug, Clone)]
pub struct RetryState {
    /// Current attempt number (0-indexed)
    pub attempt: u32,

    /// Total delay accumulated across retries
    pub total_delay: Duration,

    /// Last error encountered
    pub last_error: Option<String>,

    /// Rate limit info if detected
    pub rate_limit_info: Option<RateLimitInfo>,
}

impl RetryState {
    /// Create a new retry state
    pub fn new() -> Self {
        Self {
            attempt: 0,
            total_delay: Duration::ZERO,
            last_error: None,
            rate_limit_info: None,
        }
    }

    /// Increment attempt counter
    pub fn increment(&mut self) {
        self.attempt += 1;
    }

    /// Record a delay
    pub fn add_delay(&mut self, delay: Duration) {
        self.total_delay += delay;
    }

    /// Check if we should retry based on config
    pub fn should_retry(&self, config: &RateLimitConfig) -> bool {
        self.attempt < config.max_retries
    }

    /// Calculate delay for next retry
    pub fn next_delay(&self, config: &RateLimitConfig) -> Duration {
        // If we have a server-provided retry delay and config says to respect it
        if config.respect_retry_after {
            if let Some(ref info) = self.rate_limit_info {
                if let Some(retry_after) = info.retry_after {
                    // Add a small buffer to the server's suggestion
                    return retry_after + Duration::from_millis(100);
                }
            }
        }

        // Calculate exponential backoff
        let base_delay = config.initial_delay.as_secs_f64();
        let multiplier = config.backoff_multiplier.powi(self.attempt as i32);
        let delay_secs = base_delay * multiplier;

        // Apply jitter if enabled (±25%)
        let delay_secs = if config.add_jitter {
            let jitter = (rand_simple() - 0.5) * 0.5 * delay_secs;
            delay_secs + jitter
        } else {
            delay_secs
        };

        // Clamp to max delay
        let delay = Duration::from_secs_f64(delay_secs);
        std::cmp::min(delay, config.max_delay)
    }
}

impl Default for RetryState {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple pseudo-random number generator (0.0 to 1.0).
/// Uses current time for basic randomness without external deps.
fn rand_simple() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 1000) as f64 / 1000.0
}

/// Callback type for retry events
pub type RetryCallback = Box<dyn Fn(&RetryState, Duration) + Send + Sync>;

/// Execute an async operation with retry logic.
///
/// # Example
///
/// ```ignore
/// use aaagent::llm::rate_limit::{RateLimitConfig, with_retry};
///
/// let config = RateLimitConfig::default();
/// let result = with_retry(&config, None, || async {
///     make_api_call().await
/// }).await;
/// ```
pub async fn with_retry<F, Fut, T, E>(
    config: &RateLimitConfig,
    on_retry: Option<&RetryCallback>,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: AsRef<str> + std::fmt::Display,
{
    let mut state = RetryState::new();

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let error_str = e.to_string();

                // Try to parse rate limit info
                state.rate_limit_info = RateLimitInfo::from_gemini_error(&error_str)
                    .or_else(|| RateLimitInfo::from_openai_error(&error_str))
                    .or_else(|| RateLimitInfo::from_anthropic_error(&error_str));

                state.last_error = Some(error_str.clone());

                // Check if it's a rate limit error and we should retry
                let is_rate_limit = state.rate_limit_info.is_some()
                    || error_str.contains("429")
                    || error_str.contains("rate_limit")
                    || error_str.contains("RESOURCE_EXHAUSTED");

                if is_rate_limit && state.should_retry(config) {
                    let delay = state.next_delay(config);

                    // Notify callback
                    if let Some(callback) = on_retry {
                        callback(&state, delay);
                    }

                    // Wait before retry
                    tokio::time::sleep(delay).await;

                    state.add_delay(delay);
                    state.increment();
                } else {
                    return Err(e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_retry_delay_gemini() {
        let error = r#"Please retry in 9.528103394s."#;
        let delay = parse_retry_delay(error);
        assert!(delay.is_some());
        let secs = delay.unwrap().as_secs_f64();
        assert!(secs > 9.0 && secs < 10.0);
    }

    #[test]
    fn test_parse_retry_delay_json() {
        let error = r#"{"retryDelay": "5s"}"#;
        let delay = parse_retry_delay(error);
        assert_eq!(delay, Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_rate_limit_info_from_gemini() {
        let error = r#"HTTP 429: {"error": {"code": 429, "status": "RESOURCE_EXHAUSTED"}}"#;
        let info = RateLimitInfo::from_gemini_error(error);
        assert!(info.is_some());
    }

    #[test]
    fn test_retry_state_delay_calculation() {
        let config = RateLimitConfig::new()
            .with_initial_delay(Duration::from_secs(1))
            .with_backoff_multiplier(2.0)
            .with_jitter(false);

        let mut state = RetryState::new();

        // First retry: 1s
        let delay1 = state.next_delay(&config);
        assert_eq!(delay1, Duration::from_secs(1));

        state.increment();

        // Second retry: 2s
        let delay2 = state.next_delay(&config);
        assert_eq!(delay2, Duration::from_secs(2));

        state.increment();

        // Third retry: 4s
        let delay3 = state.next_delay(&config);
        assert_eq!(delay3, Duration::from_secs(4));
    }

    #[test]
    fn test_retry_state_respects_server_delay() {
        let config = RateLimitConfig::default();

        let mut state = RetryState::new();
        state.rate_limit_info = Some(RateLimitInfo {
            retry_after: Some(Duration::from_secs(10)),
            quota_metric: None,
            limit: None,
            message: String::new(),
        });

        let delay = state.next_delay(&config);
        // Should be server delay + 100ms buffer
        assert!(delay.as_secs() >= 10);
    }

    #[test]
    fn test_config_presets() {
        let aggressive = RateLimitConfig::aggressive();
        assert_eq!(aggressive.max_retries, 10);

        let conservative = RateLimitConfig::conservative();
        assert_eq!(conservative.max_retries, 3);

        let no_retry = RateLimitConfig::no_retry();
        assert_eq!(no_retry.max_retries, 0);
    }

    #[test]
    fn test_max_delay_cap() {
        let config = RateLimitConfig::new()
            .with_initial_delay(Duration::from_secs(10))
            .with_max_delay(Duration::from_secs(30))
            .with_backoff_multiplier(10.0)
            .with_jitter(false);

        let mut state = RetryState::new();
        state.attempt = 5; // Would be 10 * 10^5 = 1,000,000 seconds without cap

        let delay = state.next_delay(&config);
        assert_eq!(delay, Duration::from_secs(30)); // Capped to max
    }
}
