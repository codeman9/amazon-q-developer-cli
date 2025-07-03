use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eyre::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use semantic_search_client::types::{McpServerConfig as SemanticMcpServerConfig, McpToolContext};
use chrono::Utc;

use crate::cli::chat::tool_manager::{McpServerConfig, global_mcp_config_path, workspace_mcp_config_path};
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

    /// Discover all enabled MCP servers from both workspace and global configurations
    pub async fn discover_servers(&self) -> Result<Vec<McpServerInfo>> {
        let mut servers = Vec::new();
        
        // Try workspace configuration first
        if let Ok(workspace_servers) = self.discover_servers_from_scope("workspace").await {
            servers.extend(workspace_servers);
        }
        
        // Then try global configuration
        if let Ok(global_servers) = self.discover_servers_from_scope("global").await {
            servers.extend(global_servers);
        }
        
        // Filter out disabled servers
        servers.retain(|server| !server.disabled);
        
        Ok(servers)
    }

    /// Get tool schemas from MCP servers
    pub async fn get_tool_schemas(&self, servers: &[McpServerInfo]) -> Result<Vec<(McpServerInfo, ToolsListResult)>> {
        let mut results = Vec::new();
        
        for server in servers {
            match self.get_server_tool_schemas(server).await {
                Ok(tools_result) => {
                    results.push((server.clone(), tools_result));
                },
                Err(e) => {
                    tracing::warn!("Failed to get tool schemas from server {}: {:?}", server.name, e);
                    // Continue with other servers even if one fails
                }
            }
        }
        
        Ok(results)
    }

    /// Get tool schemas from a single MCP server
    async fn get_server_tool_schemas(&self, server: &McpServerInfo) -> Result<ToolsListResult> {
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

        let client = McpClient::<JsonRpcStdioTransport>::from_config(client_config)
            .with_context(|| format!("Failed to create MCP client for server {}", server.name))?;

        // Request the tools list
        let response = client.request("tools/list", None).await
            .with_context(|| format!("Failed to request tools list from server {}", server.name))?;

        if let Some(error) = response.error {
            bail!("MCP server {} returned error: {:?}", server.name, error);
        }

        let Some(result) = response.result else {
            bail!("MCP server {} returned empty result", server.name);
        };

        let tools_result = serde_json::from_value::<ToolsListResult>(result)
            .with_context(|| format!("Failed to deserialize tools list from server {}", server.name))?;

        Ok(tools_result)
    }

    /// Validate server health by attempting to connect and get basic info
    pub async fn validate_server_health(&self, server: &McpServerInfo) -> Result<bool> {
        match self.get_server_tool_schemas(server).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
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

    /// Get the count of enabled servers from all configurations
    pub async fn get_enabled_server_count(&self) -> Result<usize> {
        let servers = self.discover_servers().await?;
        Ok(servers.len())
    }

    /// Check if any MCP configuration files exist
    pub async fn has_mcp_configuration(&self) -> bool {
        let workspace_path = workspace_mcp_config_path(&self.os).unwrap_or_default();
        let global_path = global_mcp_config_path(&self.os).unwrap_or_default();
        
        self.os.fs.exists(&workspace_path) || self.os.fs.exists(&global_path)
    }

    /// Discover MCP servers from a specific scope (workspace or global)
    async fn discover_servers_from_scope(&self, scope: &str) -> Result<Vec<McpServerInfo>> {
        let config_path = match scope {
            "workspace" => workspace_mcp_config_path(&self.os)?,
            "global" => global_mcp_config_path(&self.os)?,
            _ => bail!("Invalid scope: {}. Must be 'workspace' or 'global'", scope),
        };

        self.discover_servers_from_path(&config_path, scope).await
    }

    /// Discover MCP servers from a specific configuration file path
    pub async fn discover_servers_from_path(&self, config_path: &Path, scope: &str) -> Result<Vec<McpServerInfo>> {
        println!("🔍 Discovering servers from path: {}", config_path.display());
        
        if !self.os.fs.exists(config_path) {
            println!("❌ Config file does not exist");
            return Ok(Vec::new());
        }

        println!("✅ Config file exists, loading...");
        let config = McpServerConfig::load_from_file(&self.os, config_path)
            .await
            .with_context(|| format!("Failed to load MCP config from {}", config_path.display()))?;

        println!("📊 Loaded config with {} servers", config.mcp_servers.len());
        
        let mut servers = Vec::new();
        for (name, tool_config) in config.mcp_servers {
            println!("🔧 Processing server: {} (disabled: {})", name, tool_config.disabled);
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

    /// Extract searchable content from tool schemas
    pub fn extract_searchable_content(&self, server: &McpServerInfo, tools_result: &ToolsListResult) -> Vec<String> {
        let mut searchable_content = Vec::new();
        
        for tool in &tools_result.tools {
            if let Some(content) = self.tool_to_searchable_text(server, tool) {
                searchable_content.push(content);
            }
        }
        
        searchable_content
    }

    /// Create MCP contexts from tool schemas
    pub fn create_mcp_contexts(&self, server: &McpServerInfo, tools_result: &ToolsListResult) -> Result<Vec<McpToolContext>> {
        let mut contexts = Vec::new();
        
        for tool in &tools_result.tools {
            if let Some(context) = self.tool_to_mcp_context(server, tool)? {
                contexts.push(context);
            }
        }
        
        Ok(contexts)
    }

    /// Convert a single tool JSON to searchable text
    fn tool_to_searchable_text(&self, server: &McpServerInfo, tool: &serde_json::Value) -> Option<String> {
        let tool_name = tool.get("name")?.as_str()?;
        let description = tool.get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");
        
        let parameters_text = self.extract_parameters_text(tool);
        let categories_text = self.extract_categories_text(tool);
        
        Some(format!(
            "Tool: {} from {} server. Description: {}. Parameters: {}. Categories: {}. Server command: {}",
            tool_name,
            server.name,
            description,
            parameters_text,
            categories_text,
            server.command
        ))
    }

    /// Convert a single tool JSON to McpToolContext
    fn tool_to_mcp_context(&self, server: &McpServerInfo, tool: &serde_json::Value) -> Result<Option<McpToolContext>> {
        let Some(tool_name) = tool.get("name").and_then(|n| n.as_str()) else {
            return Ok(None);
        };

        let description = tool.get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();

        let parameters = self.extract_tool_parameters(tool);
        let indexed_content = self.tool_to_searchable_text(server, tool)
            .unwrap_or_default();

        let context = McpToolContext {
            id: format!("{}_{}", server.name, tool_name),
            server_name: server.name.clone(),
            tool_name: tool_name.to_string(),
            description,
            parameters,
            server_config: server.to_semantic_config(),
            indexed_content,
            last_updated: Utc::now(),
        };

        Ok(Some(context))
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

        let required = input_schema.get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        let params: Vec<String> = properties_obj.iter().map(|(name, prop)| {
            let param_type = prop.get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");
            let is_required = required.contains(&name.as_str());
            let required_text = if is_required { ", required" } else { "" };
            
            format!("{} ({}{})", name, param_type, required_text)
        }).collect();

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
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_else(|| "general".to_string())
    }

    /// Extract structured tool parameters
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

        let required = input_schema.get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        properties_obj.iter().map(|(name, prop)| {
            let param_type = prop.get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown")
                .to_string();
            
            let description = prop.get("description")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());
            
            let is_required = required.contains(&name.as_str());

            semantic_search_client::types::ToolParameter {
                name: name.clone(),
                param_type,
                description,
                required: is_required,
            }
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

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
        
        let servers = service.discover_servers_from_path(&config_path, "workspace").await.unwrap();
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
        
        let all_servers = service.discover_servers_from_path(&config_path, "workspace").await.unwrap();
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
        
        let servers = service.discover_servers_from_path(&nonexistent_path, "workspace").await.unwrap();
        assert!(servers.is_empty());
    }

    #[tokio::test]
    async fn test_discover_servers_empty_config() {
        let empty_config = r#"{
            "mcpServers": {}
        }"#;

        let (service, _temp_dir, config_path) = create_test_service_with_config(empty_config).await;
        
        let servers = service.discover_servers_from_path(&config_path, "workspace").await.unwrap();
        assert!(servers.is_empty());
    }

    #[tokio::test]
    async fn test_discover_servers_from_invalid_json() {
        let invalid_config = r#"{ "invalid": json, missing quotes }"#;
        let (service, _temp_dir, config_path) = create_test_service_with_config(invalid_config).await;
        
        let result = service.discover_servers_from_path(&config_path, "workspace").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_schema_processor_extract_searchable_content() {
        let processor = ToolSchemaProcessor::new();
        
        let server = McpServerInfo {
            name: "test-server".to_string(),
            command: "test-command".to_string(),
            args: vec![],
            env: None,
            timeout: 30000,
            disabled: false,
            source_scope: "workspace".to_string(),
            source_path: PathBuf::from("/test/path"),
        };

        let tools_result = ToolsListResult {
            tools: vec![
                serde_json::json!({
                    "name": "get_weather",
                    "description": "Get current weather for a location",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "The location to get weather for"
                            },
                            "units": {
                                "type": "string",
                                "description": "Temperature units (celsius or fahrenheit)"
                            }
                        },
                        "required": ["location"]
                    },
                    "categories": ["weather", "api"]
                })
            ],
            next_cursor: None,
        };

        let searchable_content = processor.extract_searchable_content(&server, &tools_result);
        
        assert_eq!(searchable_content.len(), 1);
        let content = &searchable_content[0];
        assert!(content.contains("get_weather"));
        assert!(content.contains("test-server"));
        assert!(content.contains("Get current weather"));
        assert!(content.contains("location (string, required)"));
        assert!(content.contains("units (string)"));
        assert!(content.contains("weather, api"));
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
            tools: vec![
                serde_json::json!({
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
                })
            ],
            next_cursor: None,
        };

        let contexts = processor.create_mcp_contexts(&server, &tools_result).unwrap();
        
        assert_eq!(contexts.len(), 1);
        let context = &contexts[0];
        assert_eq!(context.server_name, "weather-server");
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
    async fn test_tool_schema_processor_handles_empty_tools() {
        let processor = ToolSchemaProcessor::new();
        
        let server = McpServerInfo {
            name: "empty-server".to_string(),
            command: "empty-command".to_string(),
            args: vec![],
            env: None,
            timeout: 30000,
            disabled: false,
            source_scope: "workspace".to_string(),
            source_path: PathBuf::from("/test/path"),
        };

        let tools_result = ToolsListResult {
            tools: vec![],
            next_cursor: None,
        };

        let searchable_content = processor.extract_searchable_content(&server, &tools_result);
        assert!(searchable_content.is_empty());

        let contexts = processor.create_mcp_contexts(&server, &tools_result).unwrap();
        assert!(contexts.is_empty());
    }

    #[tokio::test]
    async fn test_tool_schema_processor_handles_malformed_tools() {
        let processor = ToolSchemaProcessor::new();
        
        let server = McpServerInfo {
            name: "test-server".to_string(),
            command: "test-command".to_string(),
            args: vec![],
            env: None,
            timeout: 30000,
            disabled: false,
            source_scope: "workspace".to_string(),
            source_path: PathBuf::from("/test/path"),
        };

        let tools_result = ToolsListResult {
            tools: vec![
                serde_json::json!({
                    // Missing name field
                    "description": "A tool without a name"
                }),
                serde_json::json!({
                    "name": "valid_tool",
                    "description": "A valid tool"
                })
            ],
            next_cursor: None,
        };

        let searchable_content = processor.extract_searchable_content(&server, &tools_result);
        assert_eq!(searchable_content.len(), 1); // Only the valid tool

        let contexts = processor.create_mcp_contexts(&server, &tools_result).unwrap();
        assert_eq!(contexts.len(), 1); // Only the valid tool
        assert_eq!(contexts[0].tool_name, "valid_tool");
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
        assert_eq!(required_param.description, Some("A required string parameter".to_string()));
        
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
            }
        }
        
        assert!(parsed.is_ok());
        let config = parsed.unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
    }
}
