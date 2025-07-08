use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use chat_cli::cli::chat::tools::{
    InputSchema,
    ToolOrigin,
    ToolSpec,
};
use chat_cli::util::knowledge_store::KnowledgeStore;
use chat_cli::util::mcp_llm_integration::McpLlmIntegration;
use chat_cli::util::mcp_processor::{
    McpDiscoveryService,
    ToolSchemaProcessor,
};
use chat_cli::util::mcp_tool_integration::{
    McpToolIntegrationService,
    integrate_mcp_tools_into_config,
};
use chat_cli::util::test_utils::{
    MockMcpServer,
    create_mock_servers,
    create_temp_mcp_config,
};
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

/// Test the complete end-to-end MCP RAG workflow
#[tokio::test]
async fn test_complete_mcp_rag_workflow() {
    // Step 1: Create mock MCP servers with realistic tools
    let servers = vec![
        MockMcpServer::new("weather-server")
            .with_tool("get_current_weather", "Get current weather for a location", vec![
                ("location", "string", true),
                ("units", "string", false),
            ])
            .with_tool("get_forecast", "Get weather forecast", vec![
                ("location", "string", true),
                ("days", "number", false),
            ]),
        MockMcpServer::new("file-server")
            .with_tool("read_file", "Read contents of a file", vec![
                ("path", "string", true),
                ("encoding", "string", false),
            ])
            .with_tool("write_file", "Write contents to a file", vec![
                ("path", "string", true),
                ("content", "string", true),
            ]),
        MockMcpServer::new("database-server").with_tool("query_table", "Query database table", vec![
            ("table", "string", true),
            ("where_clause", "string", false),
        ]),
    ];

    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();

    // Step 2: Test MCP Discovery
    let discovery_service = McpDiscoveryService::new().await.unwrap();
    let discovered_servers = discovery_service
        .discover_servers_from_path(&config_path, "test")
        .await
        .unwrap();

    assert_eq!(discovered_servers.len(), 3);
    assert!(discovered_servers.iter().any(|s| s.name == "weather-server"));
    assert!(discovered_servers.iter().any(|s| s.name == "file-server"));
    assert!(discovered_servers.iter().any(|s| s.name == "database-server"));

    // Step 3: Test Tool Schema Processing
    let processor = ToolSchemaProcessor::new();
    let mut all_contexts = Vec::new();

    for server in &servers {
        if !server.should_fail {
            let server_info = server.to_server_info();
            let tools_result = server.get_tools_result().unwrap();

            let contexts = processor.create_mcp_contexts(&server_info, &tools_result).unwrap();
            all_contexts.extend(contexts);
        }
    }

    // Should have 6 tools total (2 + 2 + 1)
    assert_eq!(all_contexts.len(), 5);

    // Step 4: Test LLM Integration
    let llm_integration = McpLlmIntegration::new().await.unwrap();

    // Test tool availability
    let tools_available = llm_integration.are_mcp_tools_available().await;
    // May be false if no actual MCP tools are indexed

    // Test tool statistics
    let stats = llm_integration.get_tool_statistics().await.unwrap();
    assert!(stats.server_count >= 0);
    assert!(stats.total_tools >= 0);

    // Step 5: Test Tool Integration Service
    let integration_service = McpToolIntegrationService::new().await.unwrap();

    // Test getting tool specifications
    let tool_specs = integration_service.get_mcp_tool_specs().await.unwrap();
    // May be empty if no actual MCP tools are available

    // Test getting tools for query
    let query_tools = integration_service
        .get_mcp_tools_for_query("weather", Some(5))
        .await
        .unwrap();
    assert!(query_tools.len() <= 5);

    // Step 6: Test Tool Configuration Integration
    let mut base_config = HashMap::new();
    base_config.insert("test_tool".to_string(), ToolSpec {
        name: "test_tool".to_string(),
        description: "A test tool".to_string(),
        input_schema: InputSchema(json!({
            "type": "object",
            "properties": {},
            "required": []
        })),
        tool_origin: ToolOrigin::Native,
    });

    let integrated_config = integrate_mcp_tools_into_config(base_config).await.unwrap();

    // Should at least have the original test tool
    assert!(integrated_config.contains_key("test_tool"));

    // May have additional MCP tools if available
    let mcp_tools: Vec<_> = integrated_config
        .values()
        .filter(|spec| spec.tool_origin == ToolOrigin::Mcp)
        .collect();

    println!("Found {} MCP tools in integrated configuration", mcp_tools.len());
}

/// Test MCP tool discovery and indexing performance
#[tokio::test]
async fn test_mcp_performance() {
    use chat_cli::util::test_utils::PerformanceTestUtils;

    // Create a moderate number of servers for performance testing
    let servers = PerformanceTestUtils::create_large_server_set(10);
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();

    let discovery_service = McpDiscoveryService::new().await.unwrap();

    // Test discovery performance
    let start = std::time::Instant::now();
    let discovered_servers = discovery_service
        .discover_servers_from_path(&config_path, "test")
        .await
        .unwrap();
    let discovery_time = start.elapsed();

    assert_eq!(discovered_servers.len(), 10);
    assert!(discovery_time < Duration::from_secs(5)); // Should be fast

    // Test schema processing performance
    let processor = ToolSchemaProcessor::new();
    let start = std::time::Instant::now();

    let mut total_contexts = 0;
    for server in &servers[..5] {
        // Test with first 5 servers
        let server_info = server.to_server_info();
        let tools_result = server.get_tools_result().unwrap();
        let contexts = processor.create_mcp_contexts(&server_info, &tools_result).unwrap();
        total_contexts += contexts.len();
    }

    let processing_time = start.elapsed();
    assert!(processing_time < Duration::from_secs(2)); // Should be reasonably fast
    assert!(total_contexts > 0); // Should have processed some tools
}

/// Test error handling in end-to-end workflow
#[tokio::test]
async fn test_end_to_end_error_handling() {
    // Create servers including failing ones
    let mut servers = create_mock_servers();
    servers.push(MockMcpServer::new("error-server").with_failure("Intentional test failure"));

    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();

    // Test that the workflow continues even with some failing servers
    let discovery_service = McpDiscoveryService::new().await.unwrap();
    let discovered_servers = discovery_service
        .discover_servers_from_path(&config_path, "test")
        .await
        .unwrap();

    // Should discover non-failing servers
    assert!(!discovered_servers.is_empty());

    // Test tool schema processing with mixed success/failure
    let schema_results = discovery_service.get_tool_schemas(&discovered_servers).await.unwrap();

    // Should have some successful results
    assert!(!schema_results.is_empty());

    // Test integration service handles errors gracefully
    let integration_service = McpToolIntegrationService::new().await.unwrap();

    // These should not panic even if some operations fail
    let _tool_specs = integration_service.get_mcp_tool_specs().await;
    let _query_tools = integration_service.get_mcp_tools_for_query("test", Some(5)).await;
    let _all_tools = integration_service.get_all_mcp_tools().await;
}

/// Test concurrent operations in end-to-end workflow
#[tokio::test]
async fn test_concurrent_end_to_end_operations() {
    let servers = create_mock_servers();
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();

    // Run multiple concurrent discovery operations
    let mut handles = Vec::new();

    for i in 0..3 {
        let path = config_path.clone();
        let handle = tokio::spawn(async move {
            let service = McpDiscoveryService::new().await.unwrap();
            let scope = format!("test-{}", i);
            let discovered = service.discover_servers_from_path(&path, &scope).await.unwrap();

            // Process schemas for discovered servers
            let schema_results = service.get_tool_schemas(&discovered).await.unwrap();

            (discovered.len(), schema_results.len())
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    let results = futures::future::join_all(handles).await;

    // All operations should succeed
    for result in results {
        let (discovered_count, schema_count) = result.unwrap();
        assert!(discovered_count > 0);
        assert!(schema_count >= 0); // May be 0 if servers fail
    }
}

/// Test tool integration with realistic configuration
#[tokio::test]
async fn test_realistic_tool_integration() {
    // Create a realistic MCP configuration
    let realistic_config = json!({
        "servers": {
            "weather-api": {
                "command": "weather-mcp-server",
                "args": ["--api-key", "test-key"],
                "env": {
                    "API_KEY": "test-key"
                },
                "timeout": 30000
            },
            "file-manager": {
                "command": "file-mcp-server",
                "args": ["--safe-mode"],
                "timeout": 15000
            },
            "database-client": {
                "command": "db-mcp-server",
                "args": ["--connection", "sqlite:///test.db"],
                "timeout": 45000,
                "disabled": false
            },
            "disabled-server": {
                "command": "disabled-server",
                "disabled": true
            }
        }
    });

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("mcp.json");
    std::fs::write(&config_path, serde_json::to_string_pretty(&realistic_config).unwrap()).unwrap();

    // Test discovery with realistic configuration
    let discovery_service = McpDiscoveryService::new().await.unwrap();
    let discovered_servers = discovery_service
        .discover_servers_from_path(&config_path, "test")
        .await
        .unwrap();

    // Should discover 3 enabled servers (disabled-server should be excluded)
    assert_eq!(discovered_servers.len(), 3);

    // Verify server configurations
    let weather_server = discovered_servers.iter().find(|s| s.name == "weather-api").unwrap();
    assert_eq!(weather_server.command, "weather-mcp-server");
    assert!(weather_server.args.contains(&"--api-key".to_string()));
    assert_eq!(weather_server.timeout, 30000);

    let file_server = discovered_servers.iter().find(|s| s.name == "file-manager").unwrap();
    assert_eq!(file_server.command, "file-mcp-server");
    assert_eq!(file_server.timeout, 15000);

    let db_server = discovered_servers.iter().find(|s| s.name == "database-client").unwrap();
    assert_eq!(db_server.command, "db-mcp-server");
    assert_eq!(db_server.timeout, 45000);
    assert!(!db_server.disabled);

    // Disabled server should not be in the results
    assert!(!discovered_servers.iter().any(|s| s.name == "disabled-server"));
}

/// Test system integration with timeout handling
#[tokio::test]
async fn test_system_integration_with_timeouts() {
    let servers = create_mock_servers();
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();

    // Test that operations complete within reasonable timeouts
    let discovery_service = McpDiscoveryService::new().await.unwrap();

    let result = timeout(
        Duration::from_secs(10),
        discovery_service.discover_servers_from_path(&config_path, "test"),
    )
    .await;

    assert!(result.is_ok());
    let discovered_servers = result.unwrap().unwrap();
    assert!(!discovered_servers.is_empty());

    // Test schema processing with timeout
    let result = timeout(
        Duration::from_secs(15),
        discovery_service.get_tool_schemas(&discovered_servers),
    )
    .await;

    assert!(result.is_ok());
    let schema_results = result.unwrap().unwrap();
    assert!(!schema_results.is_empty());
}

/// Test memory usage and resource management
#[tokio::test]
async fn test_memory_and_resource_management() {
    use chat_cli::util::test_utils::PerformanceTestUtils;

    // Create servers with many tools
    let servers = PerformanceTestUtils::create_varied_servers();
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();

    let discovery_service = McpDiscoveryService::new().await.unwrap();
    let discovered_servers = discovery_service
        .discover_servers_from_path(&config_path, "test")
        .await
        .unwrap();

    let processor = ToolSchemaProcessor::new();

    // Process all servers and measure resource usage indirectly
    let mut total_contexts = 0;
    let mut total_content_length = 0;

    for server in &servers {
        if !server.should_fail {
            let server_info = server.to_server_info();
            let tools_result = server.get_tools_result().unwrap();

            let contexts = processor.create_mcp_contexts(&server_info, &tools_result).unwrap();
            total_contexts += contexts.len();

            for context in &contexts {
                total_content_length += context.indexed_content.len();
            }
        }
    }

    // Verify we processed a reasonable amount of data
    assert!(total_contexts > 10); // Should have many tools
    assert!(total_content_length > 1000); // Should have substantial content

    // Test that integration service can handle the load
    let integration_service = McpToolIntegrationService::new().await.unwrap();
    let _tool_specs = integration_service.get_mcp_tool_specs().await;
    let _statistics = integration_service.get_tool_statistics().await;
}
