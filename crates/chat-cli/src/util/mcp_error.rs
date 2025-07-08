use std::time::Duration;

use thiserror::Error;

/// Comprehensive error types for MCP operations
#[derive(Debug, Error)]
pub enum McpError {
    // Configuration errors
    #[error("MCP configuration file not found at path: {path}")]
    ConfigFileNotFound { path: String },
    
    #[error("Invalid MCP configuration: {reason}")]
    InvalidConfig { reason: String },
    
    #[error("MCP server configuration is malformed: {server_name} - {details}")]
    MalformedServerConfig { server_name: String, details: String },
    
    // Server connection errors
    #[error("Failed to connect to MCP server '{server_name}': {reason}")]
    ServerConnectionFailed { server_name: String, reason: String },
    
    #[error("MCP server '{server_name}' is unavailable (health check failed)")]
    ServerUnavailable { server_name: String },
    
    #[error("Connection to MCP server '{server_name}' timed out after {timeout:?}")]
    ServerTimeout { server_name: String, timeout: Duration },
    
    #[error("MCP server '{server_name}' returned an error: {error_message}")]
    ServerError { server_name: String, error_message: String },
    
    // Tool schema errors
    #[error("Invalid tool schema from server '{server_name}', tool '{tool_name}': {reason}")]
    InvalidToolSchema { 
        server_name: String, 
        tool_name: String, 
        reason: String 
    },
    
    #[error("Missing required field in tool schema: {field_name}")]
    MissingSchemaField { field_name: String },
    
    #[error("Tool schema parsing failed for server '{server_name}': {details}")]
    SchemaParseFailed { server_name: String, details: String },
    
    // Indexing errors
    #[error("Failed to index MCP tools from server '{server_name}': {reason}")]
    IndexingFailed { server_name: String, reason: String },
    
    #[error("Embedding generation failed for tool '{tool_name}': {reason}")]
    EmbeddingFailed { tool_name: String, reason: String },
    
    #[error("Failed to store MCP context: {reason}")]
    StorageFailed { reason: String },
    
    // Search errors
    #[error("MCP tool search failed: {reason}")]
    SearchFailed { reason: String },
    
    #[error("Invalid search query: {query} - {reason}")]
    InvalidQuery { query: String, reason: String },
    
    // LLM integration errors
    #[error("Failed to convert MCP tool to function definition: {tool_name} - {reason}")]
    FunctionDefinitionFailed { tool_name: String, reason: String },
    
    #[error("Tool parameter mapping failed for '{tool_name}': {reason}")]
    ParameterMappingFailed { tool_name: String, reason: String },
    
    // Health monitoring errors
    #[error("Health check failed for server '{server_name}': {reason}")]
    HealthCheckFailed { server_name: String, reason: String },
    
    #[error("Server '{server_name}' exceeded maximum retry attempts ({max_retries})")]
    MaxRetriesExceeded { server_name: String, max_retries: u32 },
    
    // General errors
    #[error("IO error during MCP operation: {operation} - {source}")]
    IoError { operation: String, source: std::io::Error },
    
    #[error("JSON serialization/deserialization error: {context} - {source}")]
    JsonError { context: String, source: serde_json::Error },
    
    #[error("MCP operation was cancelled: {operation}")]
    OperationCancelled { operation: String },
    
    #[error("Operation '{operation}' timed out after {timeout_seconds} seconds")]
    OperationTimeout { operation: String, timeout_seconds: u64 },
    
    #[error("Internal MCP error: {message}")]
    Internal { message: String },
    
    // Wrapped errors from other components
    #[error("MCP client error: {0}")]
    McpClient(#[from] crate::mcp_client::ClientError),
    
    #[error("Transport error: {0}")]
    Transport(#[from] crate::mcp_client::TransportError),
}

impl Clone for McpError {
    fn clone(&self) -> Self {
        match self {
            // Configuration errors
            Self::ConfigFileNotFound { path } => Self::ConfigFileNotFound { path: path.clone() },
            Self::InvalidConfig { reason } => Self::InvalidConfig { reason: reason.clone() },
            Self::MalformedServerConfig { server_name, details } => Self::MalformedServerConfig {
                server_name: server_name.clone(),
                details: details.clone(),
            },
            
            // Server connection errors
            Self::ServerConnectionFailed { server_name, reason } => Self::ServerConnectionFailed {
                server_name: server_name.clone(),
                reason: reason.clone(),
            },
            Self::ServerUnavailable { server_name } => Self::ServerUnavailable { server_name: server_name.clone() },
            Self::ServerTimeout { server_name, timeout } => Self::ServerTimeout {
                server_name: server_name.clone(),
                timeout: *timeout,
            },
            Self::ServerError { server_name, error_message } => Self::ServerError {
                server_name: server_name.clone(),
                error_message: error_message.clone(),
            },
            
            // Tool schema errors
            Self::InvalidToolSchema { server_name, tool_name, reason } => Self::InvalidToolSchema {
                server_name: server_name.clone(),
                tool_name: tool_name.clone(),
                reason: reason.clone(),
            },
            Self::MissingSchemaField { field_name } => Self::MissingSchemaField { field_name: field_name.clone() },
            Self::SchemaParseFailed { server_name, details } => Self::SchemaParseFailed {
                server_name: server_name.clone(),
                details: details.clone(),
            },
            
            // Indexing errors
            Self::IndexingFailed { server_name, reason } => Self::IndexingFailed {
                server_name: server_name.clone(),
                reason: reason.clone(),
            },
            Self::EmbeddingFailed { tool_name, reason } => Self::EmbeddingFailed {
                tool_name: tool_name.clone(),
                reason: reason.clone(),
            },
            Self::StorageFailed { reason } => Self::StorageFailed { reason: reason.clone() },
            
            // Search errors
            Self::SearchFailed { reason } => Self::SearchFailed { reason: reason.clone() },
            Self::InvalidQuery { query, reason } => Self::InvalidQuery {
                query: query.clone(),
                reason: reason.clone(),
            },
            
            // LLM integration errors
            Self::FunctionDefinitionFailed { tool_name, reason } => Self::FunctionDefinitionFailed {
                tool_name: tool_name.clone(),
                reason: reason.clone(),
            },
            Self::ParameterMappingFailed { tool_name, reason } => Self::ParameterMappingFailed {
                tool_name: tool_name.clone(),
                reason: reason.clone(),
            },
            
            // Health monitoring errors
            Self::HealthCheckFailed { server_name, reason } => Self::HealthCheckFailed {
                server_name: server_name.clone(),
                reason: reason.clone(),
            },
            Self::MaxRetriesExceeded { server_name, max_retries } => Self::MaxRetriesExceeded {
                server_name: server_name.clone(),
                max_retries: *max_retries,
            },
            
            // General errors - convert complex errors to internal since they can't be easily cloned
            Self::IoError { operation, source } => Self::Internal { 
                message: format!("IO error during {}: {}", operation, source) 
            },
            Self::JsonError { context, source } => Self::Internal { 
                message: format!("JSON error in {}: {}", context, source) 
            },
            Self::OperationCancelled { operation } => Self::OperationCancelled { operation: operation.clone() },
            Self::OperationTimeout { operation, timeout_seconds } => Self::OperationTimeout { 
                operation: operation.clone(), 
                timeout_seconds: *timeout_seconds 
            },
            Self::Internal { message } => Self::Internal { message: message.clone() },
            
            // Wrapped errors - convert to internal errors since they can't be cloned
            Self::McpClient(e) => Self::Internal { message: format!("MCP client error: {}", e) },
            Self::Transport(e) => Self::Internal { message: format!("Transport error: {}", e) },
        }
    }
}

impl McpError {
    /// Create a configuration error
    pub fn config_error(reason: impl Into<String>) -> Self {
        Self::InvalidConfig { reason: reason.into() }
    }
    
    /// Create a server connection error
    pub fn connection_failed(server_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ServerConnectionFailed {
            server_name: server_name.into(),
            reason: reason.into(),
        }
    }
    
    /// Create a tool schema error
    pub fn invalid_schema(server_name: impl Into<String>, tool_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidToolSchema {
            server_name: server_name.into(),
            tool_name: tool_name.into(),
            reason: reason.into(),
        }
    }
    
    /// Create an indexing error
    pub fn indexing_failed(server_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::IndexingFailed {
            server_name: server_name.into(),
            reason: reason.into(),
        }
    }
    
    /// Create a search error
    pub fn search_failed(reason: impl Into<String>) -> Self {
        Self::SearchFailed { reason: reason.into() }
    }
    
    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal { message: message.into() }
    }
    
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            // Retryable errors
            Self::ServerConnectionFailed { .. } |
            Self::ServerTimeout { .. } |
            Self::ServerUnavailable { .. } |
            Self::HealthCheckFailed { .. } => true,
            
            // Non-retryable errors
            Self::ConfigFileNotFound { .. } |
            Self::InvalidConfig { .. } |
            Self::MalformedServerConfig { .. } |
            Self::InvalidToolSchema { .. } |
            Self::MissingSchemaField { .. } |
            Self::SchemaParseFailed { .. } |
            Self::InvalidQuery { .. } |
            Self::FunctionDefinitionFailed { .. } |
            Self::ParameterMappingFailed { .. } |
            Self::MaxRetriesExceeded { .. } |
            Self::OperationCancelled { .. } => false,
            
            // Context-dependent errors
            Self::ServerError { .. } |
            Self::IndexingFailed { .. } |
            Self::EmbeddingFailed { .. } |
            Self::StorageFailed { .. } |
            Self::SearchFailed { .. } |
            Self::IoError { .. } |
            Self::JsonError { .. } |
            Self::OperationTimeout { .. } |
            Self::Internal { .. } => false,
            
            // Wrapped errors - delegate to source
            Self::McpClient(_) |
            Self::Transport(_) => false,
        }
    }
    
    /// Get the server name associated with this error, if any
    pub fn server_name(&self) -> Option<&str> {
        match self {
            Self::MalformedServerConfig { server_name, .. } |
            Self::ServerConnectionFailed { server_name, .. } |
            Self::ServerUnavailable { server_name } |
            Self::ServerTimeout { server_name, .. } |
            Self::ServerError { server_name, .. } |
            Self::InvalidToolSchema { server_name, .. } |
            Self::SchemaParseFailed { server_name, .. } |
            Self::IndexingFailed { server_name, .. } |
            Self::HealthCheckFailed { server_name, .. } |
            Self::MaxRetriesExceeded { server_name, .. } => Some(server_name),
            _ => None,
        }
    }
    
    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Self::ConfigFileNotFound { .. } => 
                "MCP configuration file not found. Please ensure mcp.json exists and is properly configured.".to_string(),
            
            Self::InvalidConfig { .. } => 
                "MCP configuration is invalid. Please check your mcp.json file format.".to_string(),
            
            Self::ServerConnectionFailed { server_name, .. } => 
                format!("Unable to connect to MCP server '{}'. Please check if the server is running and accessible.", server_name),
            
            Self::ServerUnavailable { server_name } => 
                format!("MCP server '{}' is currently unavailable. It may be starting up or experiencing issues.", server_name),
            
            Self::ServerTimeout { server_name, .. } => 
                format!("Connection to MCP server '{}' timed out. The server may be overloaded or unresponsive.", server_name),
            
            Self::InvalidToolSchema { server_name, tool_name, .. } => 
                format!("Tool '{}' from server '{}' has an invalid schema. The server may need to be updated.", tool_name, server_name),
            
            Self::IndexingFailed { server_name, .. } => 
                format!("Failed to index tools from server '{}'. Some tools may not be available for search.", server_name),
            
            Self::SearchFailed { .. } => 
                "Tool search failed. Please try again or check your query.".to_string(),
            
            _ => self.to_string(),
        }
    }
}

/// Result type for MCP operations
pub type McpResult<T> = Result<T, McpError>;

/// Retry configuration for MCP operations
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Create a new retry configuration
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            ..Default::default()
        }
    }
    
    /// Set the initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }
    
    /// Set the maximum delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }
    
    /// Set the backoff multiplier
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }
    
    /// Calculate delay for a given attempt
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        
        let delay_ms = self.initial_delay.as_millis() as f64 * self.backoff_multiplier.powi((attempt - 1) as i32);
        let delay = Duration::from_millis(delay_ms as u64);
        
        std::cmp::min(delay, self.max_delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_error_creation() {
        let error = McpError::connection_failed("test-server", "Connection refused");
        assert!(matches!(error, McpError::ServerConnectionFailed { .. }));
        assert_eq!(error.server_name(), Some("test-server"));
        assert!(error.is_retryable());
    }

    #[test]
    fn test_mcp_error_user_messages() {
        let error = McpError::ServerUnavailable { server_name: "weather-server".to_string() };
        let user_msg = error.user_message();
        assert!(user_msg.contains("weather-server"));
        assert!(user_msg.contains("unavailable"));
    }

    #[test]
    fn test_retry_config() {
        let config = RetryConfig::new(5)
            .with_initial_delay(Duration::from_millis(50))
            .with_backoff_multiplier(1.5);
        
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_delay, Duration::from_millis(50));
        
        // Test delay calculation
        assert_eq!(config.delay_for_attempt(0), Duration::ZERO);
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(50));
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(75));
    }

    #[test]
    fn test_error_retryability() {
        assert!(McpError::ServerConnectionFailed { 
            server_name: "test".to_string(), 
            reason: "timeout".to_string() 
        }.is_retryable());
        
        assert!(!McpError::InvalidConfig { 
            reason: "malformed json".to_string() 
        }.is_retryable());
        
        assert!(!McpError::MaxRetriesExceeded { 
            server_name: "test".to_string(), 
            max_retries: 3 
        }.is_retryable());
    }
}
