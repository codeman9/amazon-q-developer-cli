use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use tempfile::TempDir;

use chat_cli::util::{
    mcp_processor::{McpDiscoveryService, ToolSchemaProcessor, McpServerInfo},
    mcp_llm_integration::McpLlmIntegration,
    knowledge_store::KnowledgeStore,
    mcp_error::{McpError, RetryConfig},
    mcp_retry::{RetryExecutor, CircuitBreaker, HealthMonitor, CircuitState},
    test_utils::{create_mock_servers, create_temp_mcp_config, MockMcpServer, TestData},
};

/// Test comprehensive MCP server discovery workflow
#[tokio::test]
async fn test_mcp_discovery_workflow() {
    let servers = create_mock_servers();
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();
    
    let discovery_service = McpDiscoveryService::new().await.unwrap();
    let discovered_servers = discovery_service.discover_servers_from_path(&config_path, "test").await.unwrap();
    
    // Should discover non-failing servers
    assert!(discovered_servers.len() >= 3);
    
    // Verify server information
    let weather_server = discovered_servers.iter().find(|s| s.name == "weather-server");
    assert!(weather_server.is_some());
    assert_eq!(weather_server.unwrap().command, "mock-weather-server");
}

/// Test tool schema processing with various tool types
#[tokio::test]
async fn test_tool_schema_processing() {
    let processor = ToolSchemaProcessor::new();
    let server_info = McpServerInfo {
        name: "test-server".to_string(),
        command: "test-command".to_string(),
        args: vec![],
        env: Some(HashMap::new()),
        timeout: 30000,
        disabled: false,
        source_scope: "test".to_string(),
        source_path: PathBuf::from("/test"),
    };
    
    // Test with complex tool
    let complex_tool = TestData::complex_tool();
    let tools_result = chat_cli::mcp_client::ToolsListResult {
        tools: vec![complex_tool],
        next_cursor: None,
    };
    
    // Test searchable content extraction
    let searchable_content = processor.extract_searchable_content(&server_info, &tools_result);
    assert_eq!(searchable_content.len(), 1);
    assert!(searchable_content[0].contains("complex_analysis"));
    assert!(searchable_content[0].contains("test-server"));
    
    // Test MCP context creation
    let contexts = processor.create_mcp_contexts(&server_info, &tools_result).unwrap();
    assert_eq!(contexts.len(), 1);
    
    let context = &contexts[0];
    assert_eq!(context.tool_name, "complex_analysis");
    assert_eq!(context.server_name, "test-server");
    assert!(!context.parameters.is_empty());
}

/// Test error handling and retry logic
#[tokio::test]
async fn test_error_handling_and_retry() {
    // Test error creation
    let connection_error = McpError::connection_failed("test-server", "Connection refused");
    assert!(connection_error.is_retryable());
    assert_eq!(connection_error.server_name(), Some("test-server"));
    
    let config_error = McpError::config_error("Invalid configuration");
    assert!(!config_error.is_retryable());
    
    // Test retry configuration
    let retry_config = RetryConfig::new(3)
        .with_initial_delay(Duration::from_millis(100))
        .with_backoff_multiplier(2.0);
    
    assert_eq!(retry_config.max_attempts, 3);
    assert_eq!(retry_config.delay_for_attempt(0), Duration::ZERO);
    assert_eq!(retry_config.delay_for_attempt(1), Duration::from_millis(100));
    assert_eq!(retry_config.delay_for_attempt(2), Duration::from_millis(200));
    
    // Test circuit breaker
    let mut circuit_breaker = CircuitBreaker::new(2, Duration::from_secs(30));
    assert_eq!(circuit_breaker.state(), &CircuitState::Closed);
    assert!(circuit_breaker.can_execute());
    
    // Record failures to open circuit
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.state(), &CircuitState::Open);
    assert!(!circuit_breaker.can_execute());
}

/// Test health monitoring functionality
#[test]
fn test_health_monitoring() {
    let mut health_monitor = HealthMonitor::new(Duration::from_secs(60));
    
    // Initially healthy
    assert!(health_monitor.is_server_healthy("server1"));
    
    // Record failures
    health_monitor.record_server_failure("server1");
    health_monitor.record_server_failure("server1");
    health_monitor.record_server_failure("server1");
    
    // Should be unhealthy now
    assert!(!health_monitor.is_server_healthy("server1"));
    
    let unhealthy_servers = health_monitor.get_unhealthy_servers();
    assert!(unhealthy_servers.contains(&"server1".to_string()));
}

/// Test mock server functionality
#[test]
fn test_mock_server_functionality() {
    let server = MockMcpServer::new("weather-server")
        .with_tool("get_weather", "Get current weather", vec![
            ("location", "string", true),
            ("units", "string", false),
        ]);
    
    assert_eq!(server.name, "weather-server");
    assert_eq!(server.tools.len(), 1);
    
    let tools_result = server.get_tools_result().unwrap();
    assert_eq!(tools_result.tools.len(), 1);
    
    let tool = &tools_result.tools[0];
    assert_eq!(tool["name"], "get_weather");
    assert_eq!(tool["description"], "Get current weather");
    
    // Test failing server
    let failing_server = MockMcpServer::new("failing-server")
        .with_failure("Test failure");
    
    assert!(failing_server.should_fail);
    assert!(failing_server.get_tools_result().is_err());
}

/// Test configuration parsing edge cases
#[tokio::test]
async fn test_configuration_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let discovery_service = McpDiscoveryService::new().await.unwrap();
    
    // Test with minimal valid config
    let minimal_config = json!({
        "servers": {
            "minimal-server": {
                "command": "minimal-command"
            }
        }
    });
    
    let minimal_path = temp_dir.path().join("minimal.json");
    std::fs::write(&minimal_path, serde_json::to_string(&minimal_config).unwrap()).unwrap();
    
    let result = discovery_service.discover_servers_from_path(&minimal_path, "test").await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "minimal-server");
    
    // Test with non-existent file
    let non_existent = PathBuf::from("/non/existent/path/mcp.json");
    let result = discovery_service.discover_servers_from_path(&non_existent, "test").await;
    assert!(result.is_err());
    
    // Test with empty config
    let empty_config_path = temp_dir.path().join("empty.json");
    std::fs::write(&empty_config_path, "{}").unwrap();
    
    let result = discovery_service.discover_servers_from_path(&empty_config_path, "test").await.unwrap();
    assert!(result.is_empty());
}

/// Test server configuration validation
#[tokio::test]
async fn test_server_validation() {
    let discovery_service = McpDiscoveryService::new().await.unwrap();
    
    // Test valid server
    let valid_server = McpServerInfo {
        name: "valid-server".to_string(),
        command: "valid-command".to_string(),
        args: vec!["--arg".to_string()],
        env: Some(HashMap::new()),
        timeout: 30000,
        disabled: false,
        source_scope: "test".to_string(),
        source_path: PathBuf::from("/test"),
    };
    
    assert!(discovery_service.validate_server_config(&valid_server).is_ok());
    
    // Test invalid server with empty name
    let invalid_server = McpServerInfo {
        name: "".to_string(),
        command: "test-command".to_string(),
        args: vec![],
        env: Some(HashMap::new()),
        timeout: 30000,
        disabled: false,
        source_scope: "test".to_string(),
        source_path: PathBuf::from("/test"),
    };
    
    assert!(discovery_service.validate_server_config(&invalid_server).is_err());
}

/// Test LLM integration functionality
#[tokio::test]
async fn test_llm_integration() {
    let llm_integration = McpLlmIntegration::new().await.unwrap();
    
    // Test tool availability check
    let _tools_available = llm_integration.are_mcp_tools_available().await;
    
    // Test tool statistics
    let stats = llm_integration.get_tool_statistics().await.unwrap();
    assert!(stats.server_count >= 0);
    assert!(stats.total_tools >= 0);
    
    // Test tool search
    let tools = llm_integration.get_tools_for_query("test", Some(5)).await.unwrap();
    assert!(tools.len() <= 5);
}

/// Test KnowledgeStore integration
#[tokio::test]
async fn test_knowledge_store_integration() {
    let knowledge_store = KnowledgeStore::new().await.unwrap();
    
    // Test basic functionality
    let search_results = knowledge_store.search("test", None).await;
    assert!(search_results.is_ok());
    
    // Test context listing
    let contexts = knowledge_store.get_all().await;
    assert!(contexts.is_ok());
}

/// Test concurrent operations
#[tokio::test]
async fn test_concurrent_operations() {
    let servers = create_mock_servers();
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();
    
    // Run multiple discovery operations concurrently
    let mut handles = Vec::new();
    
    for i in 0..3 {
        let path = config_path.clone();
        let handle = tokio::spawn(async move {
            let service = McpDiscoveryService::new().await.unwrap();
            let scope = format!("test-{}", i);
            service.discover_servers_from_path(&path, &scope).await
        });
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    let results = futures::future::join_all(handles).await;
    
    // All operations should succeed
    for result in results {
        let servers = result.unwrap().unwrap();
        assert!(!servers.is_empty());
    }
}

/// Test performance with multiple servers
#[tokio::test]
async fn test_performance() {
    use chat_cli::util::test_utils::PerformanceTestUtils;
    
    let servers = PerformanceTestUtils::create_large_server_set(10);
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();
    
    let discovery_service = McpDiscoveryService::new().await.unwrap();
    
    let start = std::time::Instant::now();
    let discovered_servers = discovery_service.discover_servers_from_path(&config_path, "test").await.unwrap();
    let discovery_time = start.elapsed();
    
    assert_eq!(discovered_servers.len(), 10);
    assert!(discovery_time < Duration::from_secs(5)); // Should be reasonably fast
}

/// Test data consistency
#[tokio::test]
async fn test_data_consistency() {
    let servers = create_mock_servers();
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();
    
    let discovery_service = McpDiscoveryService::new().await.unwrap();
    
    // Run discovery multiple times
    let result1 = discovery_service.discover_servers_from_path(&config_path, "test").await.unwrap();
    let result2 = discovery_service.discover_servers_from_path(&config_path, "test").await.unwrap();
    
    // Results should be consistent
    assert_eq!(result1.len(), result2.len());
    
    // Server names should match
    let names1: std::collections::HashSet<_> = result1.iter().map(|s| &s.name).collect();
    let names2: std::collections::HashSet<_> = result2.iter().map(|s| &s.name).collect();
    assert_eq!(names1, names2);
}

/// Test error propagation
#[tokio::test]
async fn test_error_propagation() {
    let discovery_service = McpDiscoveryService::new().await.unwrap();
    
    // Test with non-existent file
    let non_existent = PathBuf::from("/definitely/does/not/exist/mcp.json");
    let result = discovery_service.discover_servers_from_path(&non_existent, "test").await;
    
    assert!(result.is_err());
    let error_string = result.unwrap_err().to_string();
    assert!(error_string.contains("mcp.json") || error_string.contains("file") || error_string.contains("path"));
}

/// Test tool schema edge cases
#[test]
fn test_tool_schema_edge_cases() {
    let processor = ToolSchemaProcessor::new();
    let server_info = McpServerInfo {
        name: "test-server".to_string(),
        command: "test-command".to_string(),
        args: vec![],
        env: Some(HashMap::new()),
        timeout: 30000,
        disabled: false,
        source_scope: "test".to_string(),
        source_path: PathBuf::from("/test"),
    };
    
    // Test with empty tools list
    let empty_tools_result = chat_cli::mcp_client::ToolsListResult {
        tools: vec![],
        next_cursor: None,
    };
    
    let contexts = processor.create_mcp_contexts(&server_info, &empty_tools_result).unwrap();
    assert!(contexts.is_empty());
    
    // Test with simple tool (no parameters)
    let simple_tool = TestData::simple_tool();
    let tools_result = chat_cli::mcp_client::ToolsListResult {
        tools: vec![simple_tool],
        next_cursor: None,
    };
    
    let contexts = processor.create_mcp_contexts(&server_info, &tools_result).unwrap();
    assert_eq!(contexts.len(), 1);
    assert!(contexts[0].parameters.is_empty());
}
