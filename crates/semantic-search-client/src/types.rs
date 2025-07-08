use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    Arc,
    Mutex,
};
use std::time::SystemTime;

use chrono::{
    DateTime,
    Utc,
};
use serde::{
    Deserialize,
    Serialize,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::client::SemanticContext;

/// Type alias for context ID
pub type ContextId = String;

/// Type alias for search results
pub type SearchResults = Vec<SearchResult>;

/// Type alias for context map
pub type ContextMap = HashMap<ContextId, Arc<Mutex<SemanticContext>>>;

/// A memory context containing semantic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeContext {
    /// Unique identifier for the context
    pub id: String,

    /// Human-readable name for the context
    pub name: String,

    /// Description of the context
    pub description: String,

    /// When the context was created
    pub created_at: DateTime<Utc>,

    /// When the context was last updated
    pub updated_at: DateTime<Utc>,

    /// Whether this context is persistent (saved to disk)
    pub persistent: bool,

    /// Original source path if created from a directory
    pub source_path: Option<String>,

    /// Number of items in the context
    pub item_count: usize,
}

impl KnowledgeContext {
    /// Create a new memory context
    pub fn new(
        id: String,
        name: &str,
        description: &str,
        persistent: bool,
        source_path: Option<String>,
        item_count: usize,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            name: name.to_string(),
            description: description.to_string(),
            created_at: now,
            updated_at: now,
            source_path,
            persistent,
            item_count,
        }
    }
}

/// A data point in the semantic index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    /// Unique identifier for the data point
    pub id: usize,

    /// Metadata associated with the data point
    pub payload: HashMap<String, serde_json::Value>,

    /// Vector representation of the data point
    pub vector: Vec<f32>,
}

/// A search result from the semantic index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The data point that matched
    pub point: DataPoint,

    /// Distance/similarity score (lower is better)
    pub distance: f32,
}

impl SearchResult {
    /// Create a new search result
    pub fn new(point: DataPoint, distance: f32) -> Self {
        Self { point, distance }
    }

    /// Get the text content of this result
    pub fn text(&self) -> Option<&str> {
        self.point.payload.get("text").and_then(|v| v.as_str())
    }
}

/// File type for processing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Plain text file
    Text,
    /// Markdown file
    Markdown,
    /// JSON file
    Json,
    /// Source code file (programming languages)
    Code,
    /// Unknown file type
    Unknown,
}

/// Progress status for indexing operations
#[derive(Debug, Clone)]
pub enum ProgressStatus {
    /// Counting files in the directory
    CountingFiles,
    /// Starting the indexing process with total file count
    StartingIndexing(usize),
    /// Indexing in progress with current file and total count
    Indexing(usize, usize),
    /// Creating semantic context (50% progress point)
    CreatingSemanticContext,
    /// Generating embeddings for items (50-80% progress range)
    GeneratingEmbeddings(usize, usize),
    /// Building vector index (80% progress point)
    BuildingIndex,
    /// Finalizing the index (90% progress point)
    Finalizing,
    /// Indexing complete (100% progress point)
    Complete,
}

/// Handle for tracking active operations
#[derive(Debug)]
pub struct OperationHandle {
    pub(crate) operation_type: OperationType,
    pub(crate) started_at: SystemTime,
    pub(crate) progress: Arc<tokio::sync::Mutex<ProgressInfo>>,
    pub(crate) cancel_token: CancellationToken,
    /// Task handle for proper cancellation
    pub(crate) task_handle: Option<tokio::task::AbortHandle>,
}

/// Type of operation being performed
#[derive(Debug, Clone)]
pub enum OperationType {
    /// Indexing operation with name and path
    Indexing {
        /// Display name for the operation
        name: String,
        /// Path being indexed
        path: String,
    },
    /// Clearing all contexts
    Clearing,
}

impl OperationType {
    /// Get display name for the operation
    pub fn display_name(&self) -> String {
        match self {
            OperationType::Indexing { name, .. } => format!("Indexing '{}'", name),
            OperationType::Clearing => "Clearing all".to_string(),
        }
    }
}

/// Status information for a single operation (data contract for UI)
#[derive(Debug, Clone)]
pub struct OperationStatus {
    /// Full operation ID
    pub id: String,
    /// Short operation ID (first 8 characters)
    pub short_id: String,
    /// Type of operation being performed
    pub operation_type: OperationType,
    /// When the operation started
    pub started_at: SystemTime,
    /// Current progress count
    pub current: u64,
    /// Total items to process
    pub total: u64,
    /// Current status message
    pub message: String,
    /// Whether the operation was cancelled
    pub is_cancelled: bool,
    /// Whether the operation failed
    pub is_failed: bool,
    /// Whether the operation is waiting
    pub is_waiting: bool,
    /// Estimated time to completion
    pub eta: Option<std::time::Duration>,
}

/// Overall status information (data contract for UI)
#[derive(Debug, Clone)]
pub struct SystemStatus {
    /// Total number of contexts
    pub total_contexts: usize,
    /// Number of persistent contexts
    pub persistent_contexts: usize,
    /// Number of volatile contexts
    pub volatile_contexts: usize,
    /// List of current operations
    pub operations: Vec<OperationStatus>,
    /// Number of active operations
    pub active_count: usize,
    /// Number of waiting operations
    pub waiting_count: usize,
    /// Maximum concurrent operations allowed
    pub max_concurrent: usize,
}

/// Progress information for operations
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    /// Current progress count
    pub current: u64,
    /// Total items to process
    pub total: u64,
    /// Current status message
    pub message: String,
    /// When progress tracking started
    pub progress_started_at: Option<SystemTime>,
}

impl Default for ProgressInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressInfo {
    /// Create a new progress info instance
    pub fn new() -> Self {
        Self {
            current: 0,
            total: 0,
            message: "Initializing...".to_string(),
            progress_started_at: None,
        }
    }

    /// Update progress information
    pub fn update(&mut self, current: u64, total: u64, message: String) {
        // Start tracking progress time when we first get meaningful progress
        if self.progress_started_at.is_none() && current > 0 && total > 0 {
            self.progress_started_at = Some(SystemTime::now());
        }

        self.current = current;
        self.total = total;
        self.message = message;
    }

    /// Calculate ETA based on current progress rate
    pub fn calculate_eta(&self) -> Option<std::time::Duration> {
        if let Some(started_at) = self.progress_started_at {
            if self.current > 0 && self.total > self.current {
                if let Ok(elapsed) = started_at.elapsed() {
                    let progress_rate = self.current as f64 / elapsed.as_secs_f64();
                    if progress_rate > 0.0 {
                        let remaining_items = self.total - self.current;
                        let eta_seconds = remaining_items as f64 / progress_rate;
                        return Some(std::time::Duration::from_secs_f64(eta_seconds));
                    }
                }
            }
        }
        None
    }
}

/// MCP tool context containing semantic information about an MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolContext {
    /// Unique identifier for the tool context
    pub id: String,

    /// Name of the MCP server providing this tool
    pub server_name: String,

    /// Name of the tool
    pub tool_name: String,

    /// Description of what the tool does
    pub description: String,

    /// Parameters that the tool accepts
    pub parameters: Vec<ToolParameter>,

    /// Configuration of the MCP server
    pub server_config: McpServerConfig,

    /// Searchable text representation for embeddings
    pub indexed_content: String,

    /// When this tool context was last updated
    pub last_updated: DateTime<Utc>,
}

impl McpToolContext {
    /// Create a new MCP tool context
    pub fn new(
        id: String,
        server_name: String,
        tool_name: String,
        description: String,
        parameters: Vec<ToolParameter>,
        server_config: McpServerConfig,
        indexed_content: String,
    ) -> Self {
        Self {
            id,
            server_name,
            tool_name,
            description,
            parameters,
            server_config,
            indexed_content,
            last_updated: Utc::now(),
        }
    }

    /// Convert MCP tool context to a function definition for LLM integration
    pub fn to_function_definition(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        // Convert tool parameters to JSON schema properties
        for param in &self.parameters {
            let mut param_schema = serde_json::Map::new();

            // Set parameter type
            param_schema.insert("type".to_string(), serde_json::Value::String(param.param_type.clone()));

            // Add description if available
            if let Some(description) = &param.description {
                param_schema.insert(
                    "description".to_string(),
                    serde_json::Value::String(description.clone()),
                );
            }

            properties.insert(param.name.clone(), serde_json::Value::Object(param_schema));

            // Add to required array if parameter is required
            if param.required {
                required.push(serde_json::Value::String(param.name.clone()));
            }
        }

        // Create the function definition
        serde_json::json!({
            "name": format!("{}_{}", self.server_name, self.tool_name),
            "description": self.description,
            "input_schema": {
                "type": "object",
                "properties": properties,
                "required": required
            }
        })
    }

    /// Convert MCP tool context to ToolSpec for integration with existing tool system
    pub fn to_tool_spec(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        // Convert tool parameters to JSON schema properties
        for param in &self.parameters {
            let mut param_schema = serde_json::Map::new();

            // Set parameter type
            param_schema.insert("type".to_string(), serde_json::Value::String(param.param_type.clone()));

            // Add description if available
            if let Some(description) = &param.description {
                param_schema.insert(
                    "description".to_string(),
                    serde_json::Value::String(description.clone()),
                );
            }

            properties.insert(param.name.clone(), serde_json::Value::Object(param_schema));

            // Add to required array if parameter is required
            if param.required {
                required.push(serde_json::Value::String(param.name.clone()));
            }
        }

        // Create the ToolSpec-compatible structure
        serde_json::json!({
            "name": format!("{}_{}", self.server_name, self.tool_name),
            "description": self.description,
            "inputSchema": {
                "type": "object",
                "properties": properties,
                "required": required
            },
            "toolOrigin": "mcp"
        })
    }

    /// Get the full tool name for LLM function calling
    pub fn get_function_name(&self) -> String {
        format!("{}_{}", self.server_name, self.tool_name)
    }

    /// Check if this tool matches a given query semantically
    pub fn matches_query(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();

        // Check tool name
        if self.tool_name.to_lowercase().contains(&query_lower) {
            return true;
        }

        // Check description
        if self.description.to_lowercase().contains(&query_lower) {
            return true;
        }

        // Check server name
        if self.server_name.to_lowercase().contains(&query_lower) {
            return true;
        }

        // Check parameter names and descriptions
        for param in &self.parameters {
            if param.name.to_lowercase().contains(&query_lower) {
                return true;
            }

            if let Some(desc) = &param.description {
                if desc.to_lowercase().contains(&query_lower) {
                    return true;
                }
            }
        }

        false
    }
}

/// Parameter definition for an MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    /// Name of the parameter
    pub name: String,

    /// Type of the parameter (e.g., "string", "number", "boolean")
    pub param_type: String,

    /// Optional description of the parameter
    pub description: Option<String>,

    /// Whether this parameter is required
    pub required: bool,
}

impl ToolParameter {
    /// Create a new tool parameter
    pub fn new(name: String, param_type: String, description: Option<String>, required: bool) -> Self {
        Self {
            name,
            param_type,
            description,
            required,
        }
    }
}

/// Configuration for an MCP server from mcp.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Command to execute the MCP server
    pub command: String,

    /// Arguments to pass to the command
    pub args: Vec<String>,

    /// Environment variables for the server
    pub env: Option<HashMap<String, String>>,

    /// Timeout for server operations in milliseconds
    pub timeout: u64,
}

impl McpServerConfig {
    /// Create a new MCP server configuration
    pub fn new(command: String, args: Vec<String>, env: Option<HashMap<String, String>>, timeout: u64) -> Self {
        Self {
            command,
            args,
            env,
            timeout,
        }
    }
}

/// Type of search result to distinguish between documents and MCP tools
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SearchResultType {
    /// Document-based search result
    Document,
    /// MCP tool search result
    McpTool,
}

/// Enhanced search result that can contain either document or MCP tool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedSearchResult {
    /// The original search result data
    pub base_result: SearchResult,

    /// Type of search result
    pub result_type: SearchResultType,

    /// MCP tool information if this is an MCP tool result
    pub mcp_tool_info: Option<McpToolContext>,

    /// Context name for the result
    pub context_name: String,

    /// File path if this is a document result
    pub file_path: Option<String>,

    /// Content snippet for display
    pub content: String,
}

impl EnhancedSearchResult {
    /// Create a new document search result
    pub fn new_document(
        base_result: SearchResult,
        context_name: String,
        file_path: Option<String>,
        content: String,
    ) -> Self {
        Self {
            base_result,
            result_type: SearchResultType::Document,
            mcp_tool_info: None,
            context_name,
            file_path,
            content,
        }
    }

    /// Create a new MCP tool search result
    pub fn new_mcp_tool(base_result: SearchResult, mcp_tool_info: McpToolContext, context_name: String) -> Self {
        let content = format!("Tool: {} - {}", mcp_tool_info.tool_name, mcp_tool_info.description);
        Self {
            base_result,
            result_type: SearchResultType::McpTool,
            mcp_tool_info: Some(mcp_tool_info),
            context_name,
            file_path: None,
            content,
        }
    }

    /// Get the distance/similarity score
    pub fn distance(&self) -> f32 {
        self.base_result.distance
    }
}

/// Background indexing job (internal implementation detail)
#[derive(Debug)]
pub(crate) enum IndexingJob {
    AddDirectory {
        id: Uuid,
        cancel: CancellationToken,
        path: PathBuf,
        name: String,
        description: String,
        persistent: bool,
    },
    Clear {
        id: Uuid,
        cancel: CancellationToken,
    },
}

#[cfg(test)]
mod progress_tests {
    use std::thread;

    use super::*;

    #[test]
    fn test_eta_calculation() {
        let mut progress = ProgressInfo::new();

        // No ETA initially
        assert!(progress.calculate_eta().is_none());

        // Set initial progress
        progress.update(0, 100, "Starting".to_string());
        assert!(progress.calculate_eta().is_none());

        // Simulate some progress after a small delay
        thread::sleep(std::time::Duration::from_millis(10));
        progress.update(25, 100, "25% complete".to_string());

        // Should have an ETA now
        let eta = progress.calculate_eta();
        assert!(eta.is_some());

        // ETA should be reasonable (not zero, not too large)
        if let Some(eta_duration) = eta {
            assert!(eta_duration.as_secs() < 3600); // Less than an hour
        }
    }

    #[test]
    fn test_eta_edge_cases() {
        let mut progress = ProgressInfo::new();

        // Complete progress should have no ETA
        progress.update(100, 100, "Complete".to_string());
        assert!(progress.calculate_eta().is_none());

        // Zero total should have no ETA
        progress.update(50, 0, "Invalid".to_string());
        assert!(progress.calculate_eta().is_none());
    }
}

#[cfg(test)]
mod mcp_tests {
    use super::*;

    #[test]
    fn test_tool_parameter_creation() {
        let param = ToolParameter::new(
            "location".to_string(),
            "string".to_string(),
            Some("The location to get weather for".to_string()),
            true,
        );

        assert_eq!(param.name, "location");
        assert_eq!(param.param_type, "string");
        assert_eq!(param.description, Some("The location to get weather for".to_string()));
        assert!(param.required);
    }

    #[test]
    fn test_mcp_server_config_creation() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "test-key".to_string());

        let config = McpServerConfig::new(
            "weather-server".to_string(),
            vec!["--port".to_string(), "8080".to_string()],
            Some(env.clone()),
            30000,
        );

        assert_eq!(config.command, "weather-server");
        assert_eq!(config.args, vec!["--port", "8080"]);
        assert_eq!(config.env, Some(env));
        assert_eq!(config.timeout, 30000);
    }

    #[test]
    fn test_mcp_tool_context_creation() {
        let params = vec![
            ToolParameter::new(
                "location".to_string(),
                "string".to_string(),
                Some("Location for weather".to_string()),
                true,
            ),
            ToolParameter::new(
                "units".to_string(),
                "string".to_string(),
                Some("Temperature units".to_string()),
                false,
            ),
        ];

        let server_config = McpServerConfig::new("weather-server".to_string(), vec![], None, 30000);

        let tool_context = McpToolContext::new(
            "weather-tool-1".to_string(),
            "weather-server".to_string(),
            "get_current_weather".to_string(),
            "Get current weather for a location".to_string(),
            params.clone(),
            server_config.clone(),
            "Tool: get_current_weather from weather-server. Description: Get current weather for a location. Parameters: location (string, required), units (string)".to_string(),
        );

        assert_eq!(tool_context.id, "weather-tool-1");
        assert_eq!(tool_context.server_name, "weather-server");
        assert_eq!(tool_context.tool_name, "get_current_weather");
        assert_eq!(tool_context.description, "Get current weather for a location");
        assert_eq!(tool_context.parameters.len(), 2);
        assert_eq!(tool_context.parameters[0].name, "location");
        assert!(tool_context.parameters[0].required);
        assert_eq!(tool_context.parameters[1].name, "units");
        assert!(!tool_context.parameters[1].required);
    }

    #[test]
    fn test_search_result_type_equality() {
        assert_eq!(SearchResultType::Document, SearchResultType::Document);
        assert_eq!(SearchResultType::McpTool, SearchResultType::McpTool);
        assert_ne!(SearchResultType::Document, SearchResultType::McpTool);
    }

    #[test]
    fn test_enhanced_search_result_document() {
        let mut payload = HashMap::new();
        payload.insert(
            "text".to_string(),
            serde_json::Value::String("test content".to_string()),
        );

        let data_point = DataPoint {
            id: 1,
            payload,
            vector: vec![0.1, 0.2, 0.3],
        };

        let search_result = SearchResult::new(data_point, 0.5);

        let enhanced_result = EnhancedSearchResult::new_document(
            search_result,
            "Test Context".to_string(),
            Some("/path/to/file.txt".to_string()),
            "Test content snippet".to_string(),
        );

        assert_eq!(enhanced_result.result_type, SearchResultType::Document);
        assert_eq!(enhanced_result.context_name, "Test Context");
        assert_eq!(enhanced_result.file_path, Some("/path/to/file.txt".to_string()));
        assert_eq!(enhanced_result.content, "Test content snippet");
        assert_eq!(enhanced_result.distance(), 0.5);
        assert!(enhanced_result.mcp_tool_info.is_none());
    }

    #[test]
    fn test_enhanced_search_result_mcp_tool() {
        let mut payload = HashMap::new();
        payload.insert(
            "tool_name".to_string(),
            serde_json::Value::String("get_weather".to_string()),
        );

        let data_point = DataPoint {
            id: 2,
            payload,
            vector: vec![0.4, 0.5, 0.6],
        };

        let search_result = SearchResult::new(data_point, 0.3);

        let tool_context = McpToolContext::new(
            "weather-tool-1".to_string(),
            "weather-server".to_string(),
            "get_weather".to_string(),
            "Get weather information".to_string(),
            vec![],
            McpServerConfig::new("weather-server".to_string(), vec![], None, 30000),
            "Weather tool content".to_string(),
        );

        let enhanced_result =
            EnhancedSearchResult::new_mcp_tool(search_result, tool_context.clone(), "MCP Tools".to_string());

        assert_eq!(enhanced_result.result_type, SearchResultType::McpTool);
        assert_eq!(enhanced_result.context_name, "MCP Tools");
        assert!(enhanced_result.file_path.is_none());
        assert_eq!(enhanced_result.content, "Tool: get_weather - Get weather information");
        assert_eq!(enhanced_result.distance(), 0.3);
        assert!(enhanced_result.mcp_tool_info.is_some());

        let mcp_info = enhanced_result.mcp_tool_info.unwrap();
        assert_eq!(mcp_info.tool_name, "get_weather");
        assert_eq!(mcp_info.server_name, "weather-server");
    }

    #[test]
    fn test_mcp_tool_context_serialization() {
        let tool_context = McpToolContext::new(
            "test-tool".to_string(),
            "test-server".to_string(),
            "test_function".to_string(),
            "A test function".to_string(),
            vec![],
            McpServerConfig::new("test-server".to_string(), vec![], None, 30000),
            "Test content".to_string(),
        );

        // Test serialization
        let serialized = serde_json::to_string(&tool_context).expect("Should serialize");
        assert!(serialized.contains("test-tool"));
        assert!(serialized.contains("test-server"));
        assert!(serialized.contains("test_function"));

        // Test deserialization
        let deserialized: McpToolContext = serde_json::from_str(&serialized).expect("Should deserialize");
        assert_eq!(deserialized.id, tool_context.id);
        assert_eq!(deserialized.server_name, tool_context.server_name);
        assert_eq!(deserialized.tool_name, tool_context.tool_name);
    }

    #[test]
    fn test_mcp_tool_context_to_function_definition() {
        println!("\n🔧 Testing MCP Tool Context to Function Definition");

        let tool_context = McpToolContext::new(
            "test-id".to_string(),
            "weather-server".to_string(),
            "get_weather".to_string(),
            "Get current weather for a location".to_string(),
            vec![
                ToolParameter::new(
                    "location".to_string(),
                    "string".to_string(),
                    Some("The location to get weather for".to_string()),
                    true,
                ),
                ToolParameter::new(
                    "units".to_string(),
                    "string".to_string(),
                    Some("Temperature units (celsius/fahrenheit)".to_string()),
                    false,
                ),
            ],
            McpServerConfig::new("weather-server".to_string(), vec![], None, 30000),
            "Weather tool for getting current conditions".to_string(),
        );

        let function_def = tool_context.to_function_definition();
        println!(
            "📊 Function definition: {}",
            serde_json::to_string_pretty(&function_def).unwrap()
        );

        // Verify the structure
        assert_eq!(function_def["name"], "weather-server_get_weather");
        assert_eq!(function_def["description"], "Get current weather for a location");

        let input_schema = &function_def["input_schema"];
        assert_eq!(input_schema["type"], "object");

        let properties = &input_schema["properties"];
        assert!(properties["location"].is_object());
        assert!(properties["units"].is_object());

        let required = &input_schema["required"];
        assert!(
            required
                .as_array()
                .unwrap()
                .contains(&serde_json::Value::String("location".to_string()))
        );
        assert!(
            !required
                .as_array()
                .unwrap()
                .contains(&serde_json::Value::String("units".to_string()))
        );

        println!("✅ Function definition test completed");
    }

    #[test]
    fn test_mcp_tool_context_to_tool_spec() {
        println!("\n🔧 Testing MCP Tool Context to Tool Spec");

        let tool_context = McpToolContext::new(
            "test-id".to_string(),
            "file-server".to_string(),
            "read_file".to_string(),
            "Read contents of a file".to_string(),
            vec![ToolParameter::new(
                "path".to_string(),
                "string".to_string(),
                Some("Path to the file".to_string()),
                true,
            )],
            McpServerConfig::new("file-server".to_string(), vec![], None, 30000),
            "File reading tool".to_string(),
        );

        let tool_spec = tool_context.to_tool_spec();
        println!("📊 Tool spec: {}", serde_json::to_string_pretty(&tool_spec).unwrap());

        // Verify the structure
        assert_eq!(tool_spec["name"], "file-server_read_file");
        assert_eq!(tool_spec["description"], "Read contents of a file");
        assert_eq!(tool_spec["toolOrigin"], "mcp");

        let input_schema = &tool_spec["inputSchema"];
        assert_eq!(input_schema["type"], "object");

        let properties = &input_schema["properties"];
        assert!(properties["path"].is_object());

        println!("✅ Tool spec test completed");
    }

    #[test]
    fn test_mcp_tool_context_function_name() {
        println!("\n🔧 Testing MCP Tool Context Function Name");

        let tool_context = McpToolContext::new(
            "test-id".to_string(),
            "database-server".to_string(),
            "query_table".to_string(),
            "Query a database table".to_string(),
            vec![],
            McpServerConfig::new("database-server".to_string(), vec![], None, 30000),
            "Database query tool".to_string(),
        );

        let function_name = tool_context.get_function_name();
        println!("📊 Function name: {}", function_name);

        assert_eq!(function_name, "database-server_query_table");

        println!("✅ Function name test completed");
    }

    #[test]
    fn test_mcp_tool_context_query_matching() {
        println!("\n🔧 Testing MCP Tool Context Query Matching");

        let tool_context = McpToolContext::new(
            "test-id".to_string(),
            "weather-server".to_string(),
            "get_current_weather".to_string(),
            "Get current weather conditions for any location".to_string(),
            vec![
                ToolParameter::new(
                    "location".to_string(),
                    "string".to_string(),
                    Some("Geographic location".to_string()),
                    true,
                ),
                ToolParameter::new(
                    "temperature_unit".to_string(),
                    "string".to_string(),
                    Some("Temperature measurement unit".to_string()),
                    false,
                ),
            ],
            McpServerConfig::new("weather-server".to_string(), vec![], None, 30000),
            "Weather information retrieval tool".to_string(),
        );

        // Test various query matches
        assert!(tool_context.matches_query("weather"), "Should match tool name");
        assert!(tool_context.matches_query("current"), "Should match description");
        assert!(tool_context.matches_query("location"), "Should match parameter name");
        assert!(
            tool_context.matches_query("temperature"),
            "Should match parameter description"
        );
        assert!(tool_context.matches_query("weather-server"), "Should match server name");

        // Test case insensitivity
        assert!(tool_context.matches_query("WEATHER"), "Should be case insensitive");
        assert!(tool_context.matches_query("Weather"), "Should be case insensitive");

        // Test non-matches
        assert!(
            !tool_context.matches_query("database"),
            "Should not match unrelated terms"
        );
        assert!(!tool_context.matches_query("xyz"), "Should not match random terms");

        println!("✅ Query matching test completed");
    }
}
