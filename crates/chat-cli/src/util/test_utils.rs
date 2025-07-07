use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Value};
use tempfile::TempDir;

use crate::mcp_client::ToolsListResult;
use crate::util::mcp_processor::McpServerInfo;

/// Mock MCP server for testing
#[derive(Debug, Clone)]
pub struct MockMcpServer {
    pub name: String,
    pub tools: Vec<Value>,
    pub should_fail: bool,
    pub failure_message: String,
}

impl MockMcpServer {
    /// Create a new mock MCP server
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            tools: Vec::new(),
            should_fail: false,
            failure_message: String::new(),
        }
    }
    
    /// Add a tool to the mock server
    pub fn with_tool(mut self, name: &str, description: &str, parameters: Vec<(&str, &str, bool)>) -> Self {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        
        for (param_name, param_type, is_required) in parameters {
            properties.insert(param_name.to_string(), json!({
                "type": param_type,
                "description": format!("Parameter {} for tool {}", param_name, name)
            }));
            
            if is_required {
                required.push(param_name.to_string());
            }
        }
        
        let tool = json!({
            "name": name,
            "description": description,
            "inputSchema": {
                "type": "object",
                "properties": properties,
                "required": required
            }
        });
        
        self.tools.push(tool);
        self
    }
    
    /// Make the server fail with a specific message
    pub fn with_failure(mut self, message: &str) -> Self {
        self.should_fail = true;
        self.failure_message = message.to_string();
        self
    }
    
    /// Convert to McpServerInfo for testing
    pub fn to_server_info(&self) -> McpServerInfo {
        McpServerInfo {
            name: self.name.clone(),
            command: format!("mock-{}", self.name),
            args: vec!["--test".to_string()],
            env: Some(HashMap::new()),
            timeout: 30000,
            disabled: false,
            source_scope: "test".to_string(),
            source_path: PathBuf::from("/mock/path"),
        }
    }
    
    /// Get tools list result
    pub fn get_tools_result(&self) -> Result<ToolsListResult, String> {
        if self.should_fail {
            Err(self.failure_message.clone())
        } else {
            Ok(ToolsListResult {
                tools: self.tools.clone(),
                next_cursor: None,
            })
        }
    }
}

/// Create a comprehensive set of mock MCP servers for testing
pub fn create_mock_servers() -> Vec<MockMcpServer> {
    vec![
        // Weather server with multiple tools
        MockMcpServer::new("weather-server")
            .with_tool("get_current_weather", "Get current weather for a location", vec![
                ("location", "string", true),
                ("units", "string", false),
            ])
            .with_tool("get_forecast", "Get weather forecast", vec![
                ("location", "string", true),
                ("days", "number", false),
            ]),
        
        // File server with file operations
        MockMcpServer::new("file-server")
            .with_tool("read_file", "Read contents of a file", vec![
                ("path", "string", true),
                ("encoding", "string", false),
            ])
            .with_tool("write_file", "Write contents to a file", vec![
                ("path", "string", true),
                ("content", "string", true),
                ("create_dirs", "boolean", false),
            ]),
        
        // Database server with query tools
        MockMcpServer::new("database-server")
            .with_tool("query_table", "Query database table", vec![
                ("table", "string", true),
                ("where_clause", "string", false),
                ("limit", "number", false),
            ])
            .with_tool("insert_record", "Insert record into table", vec![
                ("table", "string", true),
                ("data", "object", true),
            ]),
        
        // Simple server with minimal tools
        MockMcpServer::new("simple-server")
            .with_tool("ping", "Simple ping command", vec![]),
        
        // Failing server for error testing
        MockMcpServer::new("failing-server")
            .with_failure("Server is temporarily unavailable"),
    ]
}

/// Create mock mcp.json configuration for testing
pub fn create_mock_mcp_config(servers: &[MockMcpServer]) -> Value {
    let mut server_configs = serde_json::Map::new();
    
    for server in servers {
        if !server.should_fail {
            server_configs.insert(server.name.clone(), json!({
                "command": server.to_server_info().command,
                "args": server.to_server_info().args,
                "env": {},
                "timeout": 30000,
                "disabled": false
            }));
        }
    }
    
    json!({
        "servers": server_configs
    })
}

/// Create a temporary directory with mock mcp.json file
pub fn create_temp_mcp_config(servers: &[MockMcpServer]) -> Result<(TempDir, PathBuf), std::io::Error> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("mcp.json");
    
    let config = create_mock_mcp_config(servers);
    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
    
    Ok((temp_dir, config_path))
}

/// Test data for various MCP tool scenarios
pub struct TestData;

impl TestData {
    /// Get sample tool with complex parameters
    pub fn complex_tool() -> Value {
        json!({
            "name": "complex_analysis",
            "description": "Perform complex data analysis with multiple parameters",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "data_source": {
                        "type": "string",
                        "description": "Path to data source file"
                    },
                    "analysis_type": {
                        "type": "string",
                        "enum": ["statistical", "predictive", "descriptive"],
                        "description": "Type of analysis to perform"
                    },
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "confidence_level": {
                                "type": "number",
                                "minimum": 0.0,
                                "maximum": 1.0
                            },
                            "sample_size": {
                                "type": "integer",
                                "minimum": 1
                            }
                        }
                    },
                    "output_format": {
                        "type": "string",
                        "default": "json"
                    }
                },
                "required": ["data_source", "analysis_type"]
            }
        })
    }
    
    /// Get malformed tool for error testing
    pub fn malformed_tool() -> Value {
        json!({
            "name": "malformed_tool",
            // Missing description
            "inputSchema": {
                "type": "object",
                // Missing properties
                "required": ["missing_param"]
            }
        })
    }
    
    /// Get tool with no parameters
    pub fn simple_tool() -> Value {
        json!({
            "name": "simple_ping",
            "description": "Simple ping command with no parameters"
            // No inputSchema
        })
    }
    
    /// Get tool with invalid schema
    pub fn invalid_schema_tool() -> Value {
        json!({
            "name": "invalid_tool",
            "description": "Tool with invalid schema",
            "inputSchema": {
                "type": "invalid_type",
                "properties": "not_an_object"
            }
        })
    }
}

/// Performance test utilities
pub struct PerformanceTestUtils;

impl PerformanceTestUtils {
    /// Create a large number of mock servers for performance testing
    pub fn create_large_server_set(count: usize) -> Vec<MockMcpServer> {
        (0..count)
            .map(|i| {
                MockMcpServer::new(&format!("server-{}", i))
                    .with_tool(&format!("tool-{}-1", i), &format!("First tool for server {}", i), vec![
                        ("param1", "string", true),
                        ("param2", "number", false),
                    ])
                    .with_tool(&format!("tool-{}-2", i), &format!("Second tool for server {}", i), vec![
                        ("data", "object", true),
                    ])
            })
            .collect()
    }
    
    /// Create servers with varying tool counts for load testing
    pub fn create_varied_servers() -> Vec<MockMcpServer> {
        let mut servers = Vec::new();
        
        // Small servers (1-2 tools)
        for i in 0..5 {
            servers.push(
                MockMcpServer::new(&format!("small-server-{}", i))
                    .with_tool(&format!("tool-{}", i), "Simple tool", vec![("param", "string", true)])
            );
        }
        
        // Medium servers (5-10 tools)
        for i in 0..3 {
            let mut server = MockMcpServer::new(&format!("medium-server-{}", i));
            for j in 0..7 {
                server = server.with_tool(
                    &format!("tool-{}-{}", i, j),
                    &format!("Tool {} for medium server {}", j, i),
                    vec![("param1", "string", true), ("param2", "number", false)]
                );
            }
            servers.push(server);
        }
        
        // Large servers (20+ tools)
        for i in 0..2 {
            let mut server = MockMcpServer::new(&format!("large-server-{}", i));
            for j in 0..25 {
                server = server.with_tool(
                    &format!("tool-{}-{}", i, j),
                    &format!("Tool {} for large server {}", j, i),
                    vec![
                        ("param1", "string", true),
                        ("param2", "number", false),
                        ("param3", "boolean", false),
                    ]
                );
            }
            servers.push(server);
        }
        
        servers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_server_creation() {
        let server = MockMcpServer::new("test-server")
            .with_tool("test_tool", "A test tool", vec![("param", "string", true)]);
        
        assert_eq!(server.name, "test-server");
        assert_eq!(server.tools.len(), 1);
        assert!(!server.should_fail);
        
        let tools_result = server.get_tools_result().unwrap();
        assert_eq!(tools_result.tools.len(), 1);
        assert_eq!(tools_result.tools[0]["name"], "test_tool");
    }
    
    #[test]
    fn test_failing_server() {
        let server = MockMcpServer::new("failing-server")
            .with_failure("Test failure");
        
        assert!(server.should_fail);
        assert!(server.get_tools_result().is_err());
    }
    
    #[test]
    fn test_mock_config_creation() {
        let servers = vec![
            MockMcpServer::new("server1").with_tool("tool1", "Test tool", vec![]),
            MockMcpServer::new("server2").with_tool("tool2", "Another tool", vec![]),
        ];
        
        let config = create_mock_mcp_config(&servers);
        assert!(config["servers"]["server1"].is_object());
        assert!(config["servers"]["server2"].is_object());
    }
    
    #[test]
    fn test_temp_config_creation() {
        let servers = vec![MockMcpServer::new("test-server")];
        let (temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();
        
        assert!(config_path.exists());
        let content = std::fs::read_to_string(&config_path).unwrap();
        let config: Value = serde_json::from_str(&content).unwrap();
        assert!(config["servers"]["test-server"].is_object());
        
        drop(temp_dir); // Cleanup
    }
    
    #[test]
    fn test_performance_utils() {
        let large_set = PerformanceTestUtils::create_large_server_set(10);
        assert_eq!(large_set.len(), 10);
        
        let varied_set = PerformanceTestUtils::create_varied_servers();
        assert!(varied_set.len() > 5); // Should have small + medium + large servers
    }
    
    #[test]
    fn test_test_data() {
        let complex = TestData::complex_tool();
        assert_eq!(complex["name"], "complex_analysis");
        assert!(complex["inputSchema"]["properties"].is_object());
        
        let malformed = TestData::malformed_tool();
        assert!(malformed["description"].is_null());
        
        let simple = TestData::simple_tool();
        assert!(simple["inputSchema"].is_null());
    }
}
