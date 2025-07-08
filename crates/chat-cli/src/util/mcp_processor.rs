use std::collections::HashMap;
use std::hash::DefaultHasher;
use std::path::{
    Path,
    PathBuf,
};
use std::time::Duration;

use chrono::Utc;
use convert_case::Casing;
use eyre::{
    Context,
    Result,
    bail,
};
use semantic_search_client::types::{
    McpServerConfig as SemanticMcpServerConfig,
    McpToolContext,
};
use serde::{
    Deserialize,
    Serialize,
};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{
    debug,
    error,
    info,
    instrument,
    warn,
};

use crate::cli::chat::tool_manager::{
    McpServerConfig,
    get_valid_tool_name_regex,
    global_mcp_config_path,
    sanitize_name,
    workspace_mcp_config_path,
};
use crate::util::mcp_error::{
    McpError,
    McpResult,
    RetryConfig,
};

// Timeout constants for MCP operations
const MCP_SERVER_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);
const MCP_SERVER_CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);
/// Wrapper for timeout operations with cancellation support
async fn with_timeout_and_cancellation<T, F>(
    operation: F,
    timeout_duration: Duration,
    cancel_token: &CancellationToken,
    operation_name: &str,
) -> McpResult<T>
where
    F: std::future::Future<Output = McpResult<T>>,
{
    tokio::select! {
        result = timeout(timeout_duration, operation) => {
            match result {
                Ok(inner_result) => inner_result,
                Err(_) => Err(McpError::OperationTimeout {
                    operation: operation_name.to_string(),
                    timeout_seconds: timeout_duration.as_secs(),
                }),
            }
        }
        _ = cancel_token.cancelled() => {
            Err(McpError::OperationCancelled {
                operation: operation_name.to_string(),
            })
        }
    }
}
use super::mcp_retry::RetryExecutor;
use crate::mcp_client::{
    Client as McpClient,
    ClientConfig as McpClientConfig,
    JsonRpcStdioTransport,
    ToolsListResult,
};
use crate::os::Os;

/// Information about a discovered MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
    pub timeout: u64,
    pub disabled: bool,
    pub source_scope: String, // "workspace" or "global"
    pub source_path: PathBuf,
}

impl McpServerInfo {
    /// Convert to the semantic search client's McpServerConfig format
    pub fn to_semantic_config(&self) -> SemanticMcpServerConfig {
        SemanticMcpServerConfig {
            command: self.command.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            timeout: self.timeout,
        }
    }
}

/// Service for discovering MCP servers from configuration files
#[derive(Debug)]
pub struct McpDiscoveryService {
    os: Os,
}

impl McpDiscoveryService {
    /// Create a new MCP discovery service
    pub async fn new() -> Result<Self> {
        let os = Os::new().await.context("Failed to initialize OS interface")?;
        Ok(Self { os })
    }

    /// Discover all enabled MCP servers with options
    #[instrument(skip(self))]
    pub async fn discover_servers_with_options(&self, verbose: bool) -> McpResult<Vec<McpServerInfo>> {
        let cancel_token = CancellationToken::new();
        self.discover_servers_with_options_cancellable(verbose, &cancel_token)
            .await
    }

    /// Discover MCP servers with timeout and cancellation support
    #[instrument(skip(self, cancel_token))]
    pub async fn discover_servers_with_options_cancellable(
        &self,
        verbose: bool,
        cancel_token: &CancellationToken,
    ) -> McpResult<Vec<McpServerInfo>> {
        with_timeout_and_cancellation(
            self.discover_servers_with_options_internal(verbose),
            MCP_SERVER_DISCOVERY_TIMEOUT,
            cancel_token,
            "MCP server discovery",
        )
        .await
    }

    /// Internal discovery method without timeout wrapper
    async fn discover_servers_with_options_internal(&self, verbose: bool) -> McpResult<Vec<McpServerInfo>> {
        info!("Starting MCP server discovery");

        let mut servers = Vec::new();

        // Try workspace configuration first
        match self
            .discover_servers_from_scope_with_options("workspace", verbose)
            .await
        {
            Ok(workspace_servers) => {
                debug!("Found {} servers in workspace scope", workspace_servers.len());
                servers.extend(workspace_servers);
            },
            Err(e) => {
                warn!("Failed to discover workspace MCP servers: {}", e);
            },
        }

        // Then try global configuration
        match self.discover_servers_from_scope_with_options("global", verbose).await {
            Ok(global_servers) => {
                debug!("Found {} servers in global scope", global_servers.len());
                servers.extend(global_servers);
            },
            Err(e) => {
                warn!("Failed to discover global MCP servers: {}", e);
            },
        }

        // Filter out disabled servers
        let initial_count = servers.len();
        servers.retain(|server| !server.disabled);
        let enabled_count = servers.len();

        if initial_count > enabled_count {
            debug!("Filtered out {} disabled servers", initial_count - enabled_count);
        }

        if servers.is_empty() {
            info!("No enabled MCP servers found in any configuration");
        } else {
            info!("Discovered {} enabled MCP servers", servers.len());
        }

        Ok(servers)
    }

    /// Get tool schemas from MCP servers with retry logic
    #[instrument(skip(self, servers))]
    pub async fn get_tool_schemas(
        &self,
        servers: &[McpServerInfo],
    ) -> McpResult<Vec<(McpServerInfo, ToolsListResult)>> {
        info!("Retrieving tool schemas from {} servers", servers.len());

        let mut results = Vec::new();
        let mut retry_executor = RetryExecutor::new(RetryConfig::default());

        for server in servers {
            debug!("Getting tool schemas from server: {}", server.name);

            match retry_executor
                .execute_for_server(&server.name, "get_tool_schemas", || {
                    self.get_server_tool_schemas_with_error_handling(server)
                })
                .await
            {
                Ok(tools_result) => {
                    info!(
                        "Successfully retrieved {} tools from server '{}'",
                        tools_result.tools.len(),
                        server.name
                    );
                    results.push((server.clone(), tools_result));
                },
                Err(e) => {
                    error!("Failed to get tool schemas from server '{}': {}", server.name, e);
                    // Continue with other servers even if one fails
                },
            }
        }

        info!(
            "Successfully retrieved tool schemas from {}/{} servers",
            results.len(),
            servers.len()
        );
        Ok(results)
    }

    /// Get tool schemas from a single MCP server with proper error handling and timeout
    async fn get_server_tool_schemas_with_error_handling(&self, server: &McpServerInfo) -> McpResult<ToolsListResult> {
        let cancel_token = CancellationToken::new();
        self.get_server_tool_schemas_with_error_handling_cancellable(server, &cancel_token)
            .await
    }

    /// Get tool schemas from a single MCP server with cancellation support
    async fn get_server_tool_schemas_with_error_handling_cancellable(
        &self,
        server: &McpServerInfo,
        cancel_token: &CancellationToken,
    ) -> McpResult<ToolsListResult> {
        with_timeout_and_cancellation(
            self.get_server_tool_schemas_internal(server),
            MCP_SERVER_CONNECTION_TIMEOUT,
            cancel_token,
            &format!("MCP server connection to '{}'", server.name),
        )
        .await
    }

    /// Internal method for getting tool schemas without timeout wrapper
    async fn get_server_tool_schemas_internal(&self, server: &McpServerInfo) -> McpResult<ToolsListResult> {
        // First validate the server configuration
        self.validate_server_config(server)
            .map_err(|e| McpError::MalformedServerConfig {
                server_name: server.name.clone(),
                details: e.to_string(),
            })?;

        let client_config = McpClientConfig {
            server_name: server.name.clone(),
            bin_path: server.command.clone(),
            args: server.args.clone(),
            timeout: server.timeout,
            client_info: serde_json::json!({
                "name": "Q CLI RAG Tool Discovery",
                "version": "1.0.0"
            }),
            env: server.env.clone(),
        };

        debug!(
            "Connecting to MCP server '{}' with command: {}",
            server.name, server.command
        );

        let client = McpClient::<JsonRpcStdioTransport>::from_config(client_config)
            .map_err(|e| McpError::connection_failed(&server.name, format!("Failed to create client: {}", e)))?;

        // Request the tools list
        let response = client
            .request("tools/list", None)
            .await
            .map_err(|e| McpError::connection_failed(&server.name, format!("Failed to request tools list: {}", e)))?;

        if let Some(error) = response.error {
            return Err(McpError::ServerError {
                server_name: server.name.clone(),
                error_message: format!("Server returned error: {:?}", error),
            });
        }

        let Some(result) = response.result else {
            return Err(McpError::ServerError {
                server_name: server.name.clone(),
                error_message: "Server returned empty result".to_string(),
            });
        };

        let tools_result =
            serde_json::from_value::<ToolsListResult>(result).map_err(|e| McpError::SchemaParseFailed {
                server_name: server.name.clone(),
                details: format!("Failed to deserialize tools list: {}", e),
            })?;

        debug!(
            "Retrieved {} tools from server '{}'",
            tools_result.tools.len(),
            server.name
        );
        Ok(tools_result)
    }

    /// Validate that a server configuration is valid
    pub fn validate_server_config(&self, server: &McpServerInfo) -> Result<()> {
        if server.name.is_empty() {
            bail!("Server name cannot be empty");
        }

        if server.command.is_empty() {
            bail!("Server command cannot be empty");
        }

        if server.timeout == 0 {
            bail!("Server timeout must be greater than 0");
        }

        Ok(())
    }

    /// Discover MCP servers from a specific scope with options
    async fn discover_servers_from_scope_with_options(&self, scope: &str, verbose: bool) -> Result<Vec<McpServerInfo>> {
        let config_path = match scope {
            "workspace" => workspace_mcp_config_path(&self.os)?,
            "global" => global_mcp_config_path(&self.os)?,
            _ => bail!("Invalid scope: {}. Must be 'workspace' or 'global'", scope),
        };

        self.discover_servers_from_path_with_options(&config_path, scope, verbose)
            .await
    }

    /// Discover MCP servers from a specific configuration file path with options
    pub async fn discover_servers_from_path_with_options(
        &self,
        config_path: &Path,
        scope: &str,
        verbose: bool,
    ) -> Result<Vec<McpServerInfo>> {
        if verbose {
            println!("🔍 Discovering servers from path: {}", config_path.display());
        }

        if !self.os.fs.exists(config_path) {
            if verbose {
                println!("❌ Config file does not exist");
            }
            return Ok(Vec::new());
        }

        if verbose {
            println!("✅ Config file exists, loading...");
        }
        let config = McpServerConfig::load_from_file(&self.os, config_path)
            .await
            .with_context(|| format!("Failed to load MCP config from {}", config_path.display()))?;

        if verbose {
            println!("📊 Loaded config with {} servers", config.mcp_servers.len());
        }

        let mut servers = Vec::new();
        for (name, tool_config) in config.mcp_servers {
            if verbose {
                println!("🔧 Processing server: {} (disabled: {})", name, tool_config.disabled);
            }
            let server_info = McpServerInfo {
                name,
                command: tool_config.command,
                args: tool_config.args,
                env: tool_config.env,
                timeout: tool_config.timeout,
                disabled: tool_config.disabled,
                source_scope: scope.to_string(),
                source_path: config_path.to_path_buf(),
            };
            servers.push(server_info);
        }

        Ok(servers)
    }
}

/// Processor for transforming MCP tool schemas into searchable content
#[derive(Debug)]
pub struct ToolSchemaProcessor;

impl ToolSchemaProcessor {
    /// Create a new tool schema processor
    pub fn new() -> Self {
        Self
    }

    /// Create MCP contexts from tool schemas with comprehensive error handling
    #[instrument(skip(self, server, tools_result))]
    pub fn create_mcp_contexts(
        &self,
        server: &McpServerInfo,
        tools_result: &ToolsListResult,
    ) -> McpResult<Vec<McpToolContext>> {
        debug!(
            "Creating MCP contexts from {} tools for server '{}'",
            tools_result.tools.len(),
            server.name
        );

        let mut contexts = Vec::new();
        let mut failed_tools = Vec::new();

        for (index, tool) in tools_result.tools.iter().enumerate() {
            match self.tool_to_mcp_context(server, tool) {
                Ok(Some(context)) => {
                    contexts.push(context);
                },
                Ok(None) => {
                    let tool_name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                    warn!(
                        "Skipped tool '{}' (index {}) from server '{}' - insufficient data",
                        tool_name, index, server.name
                    );
                    failed_tools.push(tool_name.to_string());
                },
                Err(e) => {
                    let tool_name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                    error!(
                        "Failed to process tool '{}' (index {}) from server '{}': {}",
                        tool_name, index, server.name, e
                    );
                    failed_tools.push(tool_name.to_string());
                },
            }
        }

        if !failed_tools.is_empty() {
            warn!(
                "Failed to process {} tools from server '{}': {:?}",
                failed_tools.len(),
                server.name,
                failed_tools
            );
        }

        info!(
            "Successfully created {} MCP contexts from server '{}' ({} failed)",
            contexts.len(),
            server.name,
            failed_tools.len()
        );

        Ok(contexts)
    }

    /// Convert a single tool JSON to searchable text
    fn tool_to_searchable_text(&self, server: &McpServerInfo, tool: &serde_json::Value) -> Option<String> {
        let tool_name = tool.get("name")?.as_str()?;
        let description = tool.get("description").and_then(|d| d.as_str()).unwrap_or("");

        let parameters_text = self.extract_parameters_text(tool);
        let categories_text = self.extract_categories_text(tool);

        let searchable_text = format!(
            "Tool: {} from {} server. Description: {}. Parameters: {}. Categories: {}. Server command: {}",
            tool_name, server.name, description, parameters_text, categories_text, server.command
        );

        Some(searchable_text)
    }

    /// Convert a single tool JSON to McpToolContext with comprehensive error handling
    fn tool_to_mcp_context(
        &self,
        server: &McpServerInfo,
        tool: &serde_json::Value,
    ) -> McpResult<Option<McpToolContext>> {
        let Some(tool_name) = tool.get("name").and_then(|n| n.as_str()) else {
            debug!("Tool missing name field, skipping");
            return Ok(None);
        };

        // Validate required fields
        if tool_name.is_empty() {
            return Err(McpError::InvalidToolSchema {
                server_name: server.name.clone(),
                tool_name: "empty".to_string(),
                reason: "Tool name cannot be empty".to_string(),
            });
        }

        let description = tool
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();

        // Extract parameters with error handling
        let parameters = match self.extract_tool_parameters_safe(server, tool_name, tool) {
            Ok(params) => params,
            Err(e) => {
                warn!(
                    "Failed to extract parameters for tool '{}' from server '{}': {}",
                    tool_name, server.name, e
                );
                Vec::new() // Continue with empty parameters rather than failing
            },
        };

        let indexed_content = self.tool_to_searchable_text(server, tool).unwrap_or_else(|| {
            warn!(
                "Failed to generate searchable content for tool '{}' from server '{}'",
                tool_name, server.name
            );
            format!("Tool: {} from {} server", tool_name, server.name)
        });

        // Extract server name from tool name if it follows the server___tool convention
        let (normalized_server_name, clean_tool_name) = if tool_name.contains("___") {
            let parts: Vec<&str> = tool_name.splitn(2, "___").collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                // Fallback to proper server name sanitization
                let snaked_cased_name = server.name.to_case(convert_case::Case::Snake);
                let regex = get_valid_tool_name_regex();
                let mut hasher = DefaultHasher::new();
                let sanitized = sanitize_name(snaked_cased_name, &regex, &mut hasher);
                (sanitized, tool_name.to_string())
            }
        } else {
            // Use proper server name sanitization
            let snaked_cased_name = server.name.to_case(convert_case::Case::Snake);
            let regex = get_valid_tool_name_regex();
            let mut hasher = DefaultHasher::new();
            let sanitized = sanitize_name(snaked_cased_name, &regex, &mut hasher);
            (sanitized, tool_name.to_string())
        };

        let context = McpToolContext {
            id: format!("{}_{}", normalized_server_name, clean_tool_name),
            server_name: normalized_server_name,
            tool_name: clean_tool_name,
            description,
            parameters,
            server_config: server.to_semantic_config(),
            indexed_content,
            last_updated: Utc::now(),
        };

        debug!(
            "Successfully created MCP context for tool '{}' from server '{}'",
            tool_name, server.name
        );
        Ok(Some(context))
    }

    /// Extract tool parameters with comprehensive error handling
    fn extract_tool_parameters_safe(
        &self,
        server: &McpServerInfo,
        tool_name: &str,
        tool: &serde_json::Value,
    ) -> McpResult<Vec<semantic_search_client::types::ToolParameter>> {
        let Some(input_schema) = tool.get("inputSchema") else {
            debug!("Tool '{}' has no inputSchema", tool_name);
            return Ok(Vec::new());
        };

        let Some(properties) = input_schema.get("properties") else {
            debug!("Tool '{}' inputSchema has no properties", tool_name);
            return Ok(Vec::new());
        };

        let Some(properties_obj) = properties.as_object() else {
            return Err(McpError::InvalidToolSchema {
                server_name: server.name.clone(),
                tool_name: tool_name.to_string(),
                reason: "inputSchema.properties is not an object".to_string(),
            });
        };

        let required_fields: std::collections::HashSet<String> = input_schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();

        let mut parameters = Vec::new();

        for (param_name, param_def) in properties_obj {
            let param_type = param_def
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("string")
                .to_string();

            let description = param_def
                .get("description")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());

            let required = required_fields.contains(param_name);

            parameters.push(semantic_search_client::types::ToolParameter {
                name: param_name.clone(),
                param_type,
                description,
                required,
            });
        }

        debug!(
            "Extracted {} parameters for tool '{}' from server '{}'",
            parameters.len(),
            tool_name,
            server.name
        );
        Ok(parameters)
    }

    /// Extract parameters text for searchable content
    fn extract_parameters_text(&self, tool: &serde_json::Value) -> String {
        let Some(input_schema) = tool.get("inputSchema") else {
            return "no parameters".to_string();
        };

        let Some(properties) = input_schema.get("properties") else {
            return "no parameters".to_string();
        };

        let Some(properties_obj) = properties.as_object() else {
            return "no parameters".to_string();
        };

        let required = input_schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        let params: Vec<String> = properties_obj
            .iter()
            .map(|(name, prop)| {
                let param_type = prop.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                let is_required = required.contains(&name.as_str());
                let required_text = if is_required { ", required" } else { "" };

                format!("{} ({}{})", name, param_type, required_text)
            })
            .collect();

        if params.is_empty() {
            "no parameters".to_string()
        } else {
            params.join(", ")
        }
    }

    /// Extract categories text for searchable content
    fn extract_categories_text(&self, tool: &serde_json::Value) -> String {
        tool.get("categories")
            .and_then(|c| c.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
            .unwrap_or_else(|| "general".to_string())
    }

    /// Extract structured tool parameters
    /// Extract tool parameters from a tool schema (for future use)
    #[allow(dead_code)]
    fn extract_tool_parameters(&self, tool: &serde_json::Value) -> Vec<semantic_search_client::types::ToolParameter> {
        let Some(input_schema) = tool.get("inputSchema") else {
            return Vec::new();
        };

        let Some(properties) = input_schema.get("properties") else {
            return Vec::new();
        };

        let Some(properties_obj) = properties.as_object() else {
            return Vec::new();
        };

        let required = input_schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        properties_obj
            .iter()
            .map(|(name, prop)| {
                let param_type = prop
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let description = prop.get("description").and_then(|d| d.as_str()).map(|s| s.to_string());

                let is_required = required.contains(&name.as_str());

                semantic_search_client::types::ToolParameter {
                    name: name.clone(),
                    param_type,
                    description,
                    required: is_required,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tempfile::TempDir;

    use super::*;

    async fn create_test_service_with_config(config_content: &str) -> (McpDiscoveryService, TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("mcp.json");

        let service = McpDiscoveryService::new().await.unwrap();

        // Ensure the directory exists
        if let Some(parent) = config_path.parent() {
            service.os.fs.create_dir_all(parent).await.unwrap();
        }

        // Use the Os filesystem interface to write the file
        service.os.fs.write(&config_path, config_content).await.unwrap();

        (service, temp_dir, config_path)
    }

    #[tokio::test]
    async fn test_discover_servers_from_valid_config() {
        let config_content = r#"{
            "mcpServers": {
                "weather-server": {
                    "command": "weather-mcp-server",
                    "args": ["--api-key", "test"],
                    "env": {"API_KEY": "test123"},
                    "timeout": 30000,
                    "disabled": false
                },
                "file-server": {
                    "command": "file-mcp-server",
                    "args": [],
                    "timeout": 15000,
                    "disabled": false
                }
            }
        }"#;

        let (service, _temp_dir, config_path) = create_test_service_with_config(config_content).await;

        let servers = service
            .discover_servers_from_path_with_options(&config_path, "workspace", false)
            .await
            .unwrap();
        assert_eq!(servers.len(), 2);

        let weather_server = servers.iter().find(|s| s.name == "weather-server").unwrap();
        assert_eq!(weather_server.command, "weather-mcp-server");
        assert_eq!(weather_server.args, vec!["--api-key", "test"]);
        assert_eq!(weather_server.env.as_ref().unwrap().get("API_KEY").unwrap(), "test123");
        assert_eq!(weather_server.timeout, 30000);
        assert!(!weather_server.disabled);

        let file_server = servers.iter().find(|s| s.name == "file-server").unwrap();
        assert_eq!(file_server.command, "file-mcp-server");
        assert!(file_server.args.is_empty());
        assert_eq!(file_server.timeout, 15000);
    }

    #[tokio::test]
    async fn test_discover_servers_filters_disabled() {
        let config_content = r#"{
            "mcpServers": {
                "enabled-server": {
                    "command": "enabled-command",
                    "args": [],
                    "timeout": 30000,
                    "disabled": false
                },
                "disabled-server": {
                    "command": "disabled-command",
                    "args": [],
                    "timeout": 30000,
                    "disabled": true
                }
            }
        }"#;

        let (service, _temp_dir, config_path) = create_test_service_with_config(config_content).await;

        let all_servers = service
            .discover_servers_from_path_with_options(&config_path, "workspace", false)
            .await
            .unwrap();
        assert_eq!(all_servers.len(), 2);

        // The discover_servers method should filter out disabled servers
        // But discover_servers_from_path returns all servers, filtering happens in discover_servers
        let enabled_server = all_servers.iter().find(|s| s.name == "enabled-server").unwrap();
        let disabled_server = all_servers.iter().find(|s| s.name == "disabled-server").unwrap();

        assert!(!enabled_server.disabled);
        assert!(disabled_server.disabled);
    }

    #[tokio::test]
    async fn test_discover_servers_from_nonexistent_file() {
        let service = McpDiscoveryService::new().await.unwrap();
        let nonexistent_path = PathBuf::from("/nonexistent/path/mcp.json");

        let servers = service
            .discover_servers_from_path_with_options(&nonexistent_path, "workspace", false)
            .await
            .unwrap();
        assert!(servers.is_empty());
    }

    #[tokio::test]
    async fn test_discover_servers_empty_config() {
        let empty_config = r#"{
            "mcpServers": {}
        }"#;

        let (service, _temp_dir, config_path) = create_test_service_with_config(empty_config).await;

        let servers = service
            .discover_servers_from_path_with_options(&config_path, "workspace", false)
            .await
            .unwrap();
        assert!(servers.is_empty());
    }

    #[tokio::test]
    async fn test_discover_servers_from_invalid_json() {
        let invalid_config = r#"{ "invalid": json, missing quotes }"#;
        let (service, _temp_dir, config_path) = create_test_service_with_config(invalid_config).await;

        let result = service
            .discover_servers_from_path_with_options(&config_path, "workspace", false)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_schema_processor_create_mcp_contexts() {
        let processor = ToolSchemaProcessor::new();

        let server = McpServerInfo {
            name: "weather-server".to_string(),
            command: "weather-mcp-server".to_string(),
            args: vec!["--port".to_string(), "3000".to_string()],
            env: Some([("API_KEY".to_string(), "test123".to_string())].into()),
            timeout: 45000,
            disabled: false,
            source_scope: "global".to_string(),
            source_path: PathBuf::from("/global/mcp.json"),
        };

        let tools_result = ToolsListResult {
            tools: vec![serde_json::json!({
                "name": "forecast",
                "description": "Get weather forecast",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "city": {
                            "type": "string",
                            "description": "City name"
                        },
                        "days": {
                            "type": "number",
                            "description": "Number of days"
                        }
                    },
                    "required": ["city"]
                }
            })],
            next_cursor: None,
        };

        let contexts = processor.create_mcp_contexts(&server, &tools_result).unwrap();

        assert_eq!(contexts.len(), 1);
        let context = &contexts[0];
        assert_eq!(context.server_name, "weather_server");
        assert_eq!(context.tool_name, "forecast");
        assert_eq!(context.description, "Get weather forecast");
        assert_eq!(context.parameters.len(), 2);

        let city_param = context.parameters.iter().find(|p| p.name == "city").unwrap();
        assert_eq!(city_param.param_type, "string");
        assert_eq!(city_param.description, Some("City name".to_string()));
        assert!(city_param.required);

        let days_param = context.parameters.iter().find(|p| p.name == "days").unwrap();
        assert_eq!(days_param.param_type, "number");
        assert!(!days_param.required);
    }

    #[tokio::test]
    async fn test_tool_schema_processor_parameter_extraction() {
        let processor = ToolSchemaProcessor::new();

        let tool_with_complex_params = serde_json::json!({
            "name": "complex_tool",
            "description": "A tool with complex parameters",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "required_string": {
                        "type": "string",
                        "description": "A required string parameter"
                    },
                    "optional_number": {
                        "type": "number",
                        "description": "An optional number parameter"
                    },
                    "boolean_flag": {
                        "type": "boolean"
                        // No description
                    }
                },
                "required": ["required_string"]
            }
        });

        let params_text = processor.extract_parameters_text(&tool_with_complex_params);
        assert!(params_text.contains("required_string (string, required)"));
        assert!(params_text.contains("optional_number (number)"));
        assert!(params_text.contains("boolean_flag (boolean)"));
        assert!(!params_text.contains("boolean_flag (boolean, required)"));

        let structured_params = processor.extract_tool_parameters(&tool_with_complex_params);
        assert_eq!(structured_params.len(), 3);

        let required_param = structured_params.iter().find(|p| p.name == "required_string").unwrap();
        assert!(required_param.required);
        assert_eq!(
            required_param.description,
            Some("A required string parameter".to_string())
        );

        let optional_param = structured_params.iter().find(|p| p.name == "optional_number").unwrap();
        assert!(!optional_param.required);

        let boolean_param = structured_params.iter().find(|p| p.name == "boolean_flag").unwrap();
        assert_eq!(boolean_param.description, None);
    }

    #[tokio::test]
    async fn test_validate_server_config() {
        let service = McpDiscoveryService::new().await.unwrap();

        // Valid server
        let valid_server = McpServerInfo {
            name: "test-server".to_string(),
            command: "test-command".to_string(),
            args: vec!["arg1".to_string()],
            env: None,
            timeout: 30000,
            disabled: false,
            source_scope: "workspace".to_string(),
            source_path: PathBuf::from("/test/path"),
        };
        assert!(service.validate_server_config(&valid_server).is_ok());

        // Invalid server - empty name
        let invalid_server = McpServerInfo {
            name: "".to_string(),
            command: "test-command".to_string(),
            args: vec![],
            env: None,
            timeout: 30000,
            disabled: false,
            source_scope: "workspace".to_string(),
            source_path: PathBuf::from("/test/path"),
        };
        assert!(service.validate_server_config(&invalid_server).is_err());

        // Invalid server - empty command
        let invalid_server = McpServerInfo {
            name: "test-server".to_string(),
            command: "".to_string(),
            args: vec![],
            env: None,
            timeout: 30000,
            disabled: false,
            source_scope: "workspace".to_string(),
            source_path: PathBuf::from("/test/path"),
        };
        assert!(service.validate_server_config(&invalid_server).is_err());

        // Invalid server - zero timeout
        let invalid_server = McpServerInfo {
            name: "test-server".to_string(),
            command: "test-command".to_string(),
            args: vec![],
            env: None,
            timeout: 0,
            disabled: false,
            source_scope: "workspace".to_string(),
            source_path: PathBuf::from("/test/path"),
        };
        assert!(service.validate_server_config(&invalid_server).is_err());
    }

    #[tokio::test]
    async fn test_mcp_server_info_to_semantic_config() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let server_info = McpServerInfo {
            name: "test-server".to_string(),
            command: "test-command".to_string(),
            args: vec!["arg1".to_string(), "arg2".to_string()],
            env: Some(env.clone()),
            timeout: 45000,
            disabled: false,
            source_scope: "global".to_string(),
            source_path: PathBuf::from("/global/mcp.json"),
        };

        let semantic_config = server_info.to_semantic_config();

        assert_eq!(semantic_config.command, "test-command");
        assert_eq!(semantic_config.args, vec!["arg1", "arg2"]);
        assert_eq!(semantic_config.env, Some(env));
        assert_eq!(semantic_config.timeout, 45000);
    }

    #[tokio::test]
    async fn test_json_parsing_directly() {
        let config_content = r#"{
            "mcpServers": {
                "weather-server": {
                    "command": "weather-mcp-server",
                    "args": ["--api-key", "test"],
                    "env": {"API_KEY": "test123"},
                    "timeout": 30000,
                    "disabled": false
                }
            }
        }"#;

        // Test direct JSON parsing
        let parsed: Result<McpServerConfig, _> = serde_json::from_str(config_content);
        match &parsed {
            Ok(config) => {
                println!("Direct JSON parsing successful");
                println!("Config: {:?}", config);
                println!("Number of servers: {}", config.mcp_servers.len());
            },
            Err(e) => {
                println!("Direct JSON parsing failed: {:?}", e);
            },
        }

        assert!(parsed.is_ok());
        let config = parsed.unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
    }
}
