use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eyre::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use semantic_search_client::types::McpServerConfig as SemanticMcpServerConfig;

use crate::cli::chat::tool_manager::{McpServerConfig, global_mcp_config_path, workspace_mcp_config_path};
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
    async fn discover_servers_from_path(&self, config_path: &Path, scope: &str) -> Result<Vec<McpServerInfo>> {
        if !self.os.fs.exists(config_path) {
            return Ok(Vec::new());
        }

        let config = McpServerConfig::load_from_file(&self.os, config_path)
            .await
            .with_context(|| format!("Failed to load MCP config from {}", config_path.display()))?;

        let mut servers = Vec::new();
        for (name, tool_config) in config.mcp_servers {
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
