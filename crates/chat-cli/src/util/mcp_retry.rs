use std::future::Future;
use std::time::{
    Duration,
    Instant,
};

use tokio::time::sleep;
use tracing::{
    debug,
    error,
    warn,
};

use super::mcp_error::{
    McpError,
    McpResult,
    RetryConfig,
};

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, reject requests
    HalfOpen, // Testing if service recovered
}

/// Circuit breaker for MCP operations
#[derive(Debug)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    failure_threshold: u32,
    recovery_timeout: Duration,
    last_failure_time: Option<Instant>,
    success_threshold: u32,
    half_open_successes: u32,
}

impl CircuitBreaker {
    /// Check if the circuit breaker allows the operation
    pub fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() >= self.recovery_timeout {
                        debug!("Circuit breaker transitioning to half-open state");
                        self.state = CircuitState::HalfOpen;
                        self.half_open_successes = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful operation
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count = 0;
            },
            CircuitState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.success_threshold {
                    debug!("Circuit breaker transitioning to closed state after recovery");
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.last_failure_time = None;
                }
            },
            CircuitState::Open => {
                // Should not happen, but reset if it does
                self.state = CircuitState::Closed;
                self.failure_count = 0;
                self.last_failure_time = None;
            },
        }
    }

    /// Record a failed operation
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    warn!("Circuit breaker opening due to {} failures", self.failure_count);
                    self.state = CircuitState::Open;
                }
            },
            CircuitState::HalfOpen => {
                warn!("Circuit breaker returning to open state after failure during recovery");
                self.state = CircuitState::Open;
                self.half_open_successes = 0;
            },
            CircuitState::Open => {
                // Already open, just update the failure time
            },
        }
    }
}

/// Retry executor with exponential backoff and circuit breaker
pub struct RetryExecutor {
    config: RetryConfig,
    circuit_breaker: Option<CircuitBreaker>,
}

impl RetryExecutor {
    /// Create a new retry executor
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            circuit_breaker: None,
        }
    }

    /// Execute an operation with retry logic
    pub async fn execute<F, Fut, T>(&mut self, operation_name: &str, operation: F) -> McpResult<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = McpResult<T>>,
    {
        // Check circuit breaker
        if let Some(ref mut cb) = self.circuit_breaker {
            if !cb.can_execute() {
                return Err(McpError::OperationCancelled {
                    operation: format!("{} (circuit breaker open)", operation_name),
                });
            }
        }

        let mut last_error = None;

        for attempt in 0..self.config.max_attempts {
            if attempt > 0 {
                let delay = self.config.delay_for_attempt(attempt);
                debug!(
                    "Retrying {} (attempt {}/{}) after {:?}",
                    operation_name,
                    attempt + 1,
                    self.config.max_attempts,
                    delay
                );
                sleep(delay).await;
            }

            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        debug!("Operation {} succeeded after {} retries", operation_name, attempt);
                    }

                    // Record success in circuit breaker
                    if let Some(ref mut cb) = self.circuit_breaker {
                        cb.record_success();
                    }

                    return Ok(result);
                },
                Err(error) => {
                    last_error = Some(error.clone());

                    // Check if error is retryable
                    if !error.is_retryable() {
                        debug!(
                            "Operation {} failed with non-retryable error: {}",
                            operation_name, error
                        );

                        // Record failure in circuit breaker for non-retryable errors too
                        if let Some(ref mut cb) = self.circuit_breaker {
                            cb.record_failure();
                        }

                        return Err(error);
                    }

                    warn!(
                        "Operation {} failed (attempt {}/{}): {}",
                        operation_name,
                        attempt + 1,
                        self.config.max_attempts,
                        error
                    );
                },
            }
        }

        // All attempts failed
        let final_error = last_error.unwrap_or_else(|| McpError::internal("Unknown error"));

        // Record failure in circuit breaker
        if let Some(ref mut cb) = self.circuit_breaker {
            cb.record_failure();
        }

        error!(
            "Operation {} failed after {} attempts: {}",
            operation_name, self.config.max_attempts, final_error
        );

        // Convert to max retries exceeded error if it was retryable
        if final_error.is_retryable() {
            if let Some(server_name) = final_error.server_name() {
                Err(McpError::MaxRetriesExceeded {
                    server_name: server_name.to_string(),
                    max_retries: self.config.max_attempts,
                })
            } else {
                Err(final_error)
            }
        } else {
            Err(final_error)
        }
    }

    /// Execute an operation with a specific server context
    pub async fn execute_for_server<F, Fut, T>(
        &mut self,
        server_name: &str,
        operation_name: &str,
        operation: F,
    ) -> McpResult<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = McpResult<T>>,
    {
        let full_operation_name = format!("{}:{}", server_name, operation_name);
        self.execute(&full_operation_name, operation).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        Mutex,
    };

    use super::*;

    #[test]
    fn test_circuit_breaker_states() {
        let mut cb = CircuitBreaker::new(2, Duration::from_secs(5));

        // Initially closed
        assert_eq!(cb.state(), &CircuitState::Closed);
        assert!(cb.can_execute());

        // First failure
        cb.record_failure();
        assert_eq!(cb.state(), &CircuitState::Closed);
        assert!(cb.can_execute());

        // Second failure - should open
        cb.record_failure();
        assert_eq!(cb.state(), &CircuitState::Open);
        assert!(!cb.can_execute());
    }

    #[tokio::test]
    async fn test_retry_executor_success() {
        let config = RetryConfig::new(3);
        let mut executor = RetryExecutor::new(config);

        let attempt_count = Arc::new(Mutex::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result: McpResult<&str> = executor
            .execute("test_op", || {
                let count = attempt_count_clone.clone();
                async move {
                    let mut guard = count.lock().unwrap();
                    *guard += 1;
                    let current_attempt = *guard;
                    drop(guard);

                    if current_attempt < 2 {
                        Err(McpError::connection_failed("test-server", "Connection refused"))
                    } else {
                        Ok("success")
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(*attempt_count.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_retry_executor_max_retries() {
        let config = RetryConfig::new(2);
        let mut executor = RetryExecutor::new(config);

        let attempt_count = Arc::new(Mutex::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result: McpResult<&str> = executor
            .execute("test_op", || {
                let count = attempt_count_clone.clone();
                async move {
                    let mut guard = count.lock().unwrap();
                    *guard += 1;
                    drop(guard);

                    Err(McpError::connection_failed("test-server", "Connection refused"))
                }
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::MaxRetriesExceeded { .. }));
        assert_eq!(*attempt_count.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_retry_executor_non_retryable() {
        let config = RetryConfig::new(3);
        let mut executor = RetryExecutor::new(config);

        let attempt_count = Arc::new(Mutex::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result: McpResult<&str> = executor
            .execute("test_op", || {
                let count = attempt_count_clone.clone();
                async move {
                    let mut guard = count.lock().unwrap();
                    *guard += 1;
                    drop(guard);

                    Err(McpError::InvalidConfig {
                        reason: "Bad config".to_string(),
                    })
                }
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidConfig { .. }));
        assert_eq!(*attempt_count.lock().unwrap(), 1); // Should not retry non-retryable errors
    }

    #[test]
    fn test_health_monitor() {
        let mut monitor = HealthMonitor::new(Duration::from_secs(10));

        // Initially healthy
        assert!(monitor.is_server_healthy("test-server"));

        // Record failures
        monitor.record_server_failure("test-server");
        monitor.record_server_failure("test-server");
        monitor.record_server_failure("test-server");

        // Should be unhealthy now
        assert!(!monitor.is_server_healthy("test-server"));

        let unhealthy = monitor.get_unhealthy_servers();
        assert!(unhealthy.contains(&"test-server".to_string()));
    }
}
