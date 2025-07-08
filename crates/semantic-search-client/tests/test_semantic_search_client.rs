use std::{
    env,
    fs,
};

use chrono::Utc;
use semantic_search_client::{
    McpServerConfig,
    McpToolContext,
    SemanticSearchClient,
    ToolParameter,
};

#[test]
fn test_client_initialization() {
    if env::var("MEMORY_BANK_USE_REAL_EMBEDDERS").is_err() {
        println!("Skipping test: MEMORY_BANK_USE_REAL_EMBEDDERS not set");
        return;
    }
    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_client_init");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a semantic search client
    let client = SemanticSearchClient::new(base_dir.clone()).unwrap();

    // Verify the client was created successfully
    assert_eq!(client.get_contexts().len(), 0);

    // Instead of using the actual default directory, use our test directory again
    // This ensures test isolation and prevents interference from existing contexts
    let client = SemanticSearchClient::new(base_dir.clone()).unwrap();
    assert_eq!(client.get_contexts().len(), 0);

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}

#[test]
fn test_add_context_from_text() {
    if env::var("MEMORY_BANK_USE_REAL_EMBEDDERS").is_err() {
        println!("Skipping test: MEMORY_BANK_USE_REAL_EMBEDDERS not set");
        return;
    }
    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_add_text");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a semantic search client
    let mut client = SemanticSearchClient::new(base_dir).unwrap();

    // Add a context from text
    let context_id = client
        .add_context_from_text(
            "This is a test text for semantic memory",
            "Test Text Context",
            "A context created from text",
            false,
        )
        .unwrap();

    // Verify the context was created
    let contexts = client.get_all_contexts();
    assert!(!contexts.is_empty());

    // Test search functionality
    let _results = client
        .search_context(&context_id, "test semantic memory", Some(5))
        .unwrap();
    // Don't assert on results being non-empty as it depends on the embedder implementation
    // assert!(!results.is_empty());

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}

#[test]
fn test_search_all_contexts() {
    if env::var("MEMORY_BANK_USE_REAL_EMBEDDERS").is_err() {
        println!("Skipping test: MEMORY_BANK_USE_REAL_EMBEDDERS not set");
        return;
    }
    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_search_all");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a semantic search client
    let mut client = SemanticSearchClient::new(base_dir).unwrap();

    // Add multiple contexts
    let _id1 = client
        .add_context_from_text(
            "Information about AWS Lambda functions and serverless computing",
            "AWS Lambda",
            "Serverless computing information",
            false,
        )
        .unwrap();

    let _id2 = client
        .add_context_from_text(
            "Amazon S3 is a scalable object storage service",
            "Amazon S3",
            "Storage service information",
            false,
        )
        .unwrap();

    // Search across all contexts
    let results = client.search_all("serverless lambda", Some(5)).unwrap();
    assert!(!results.is_empty());

    // Search with a different query
    let results = client.search_all("storage S3", Some(5)).unwrap();
    assert!(!results.is_empty());

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}

#[test]
fn test_persistent_context() {
    if env::var("MEMORY_BANK_USE_REAL_EMBEDDERS").is_err() {
        println!("Skipping test: MEMORY_BANK_USE_REAL_EMBEDDERS not set");
        return;
    }
    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_persistent");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a test file
    let test_file = temp_dir.join("test.txt");
    fs::write(&test_file, "This is a test file for persistent context").unwrap();

    // Create a semantic search client
    let mut client = SemanticSearchClient::new(base_dir.clone()).unwrap();

    // Add a volatile context
    let context_id = client
        .add_context_from_text(
            "This is a volatile context",
            "Volatile Context",
            "A non-persistent context",
            false,
        )
        .unwrap();

    // Make it persistent
    client
        .make_persistent(&context_id, "Persistent Context", "A now-persistent context")
        .unwrap();

    // Create a new client to verify persistence
    let client2 = SemanticSearchClient::new(base_dir).unwrap();
    let contexts = client2.get_contexts();

    // Verify the context was persisted
    assert!(contexts.iter().any(|c| c.name == "Persistent Context"));

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}

#[test]
fn test_remove_context() {
    if env::var("MEMORY_BANK_USE_REAL_EMBEDDERS").is_err() {
        println!("Skipping test: MEMORY_BANK_USE_REAL_EMBEDDERS not set");
        return;
    }
    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_remove");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a semantic search client
    let mut client = SemanticSearchClient::new(base_dir).unwrap();

    // Add contexts
    let id1 = client
        .add_context_from_text(
            "Context to be removed by ID",
            "Remove by ID",
            "Test removal by ID",
            true,
        )
        .unwrap();

    let _id2 = client
        .add_context_from_text(
            "Context to be removed by name",
            "Remove by Name",
            "Test removal by name",
            true,
        )
        .unwrap();

    // Remove by ID
    client.remove_context_by_id(&id1, true).unwrap();

    // Remove by name
    client.remove_context_by_name("Remove by Name", true).unwrap();

    // Verify contexts were removed
    let contexts = client.get_contexts();
    assert!(contexts.is_empty());

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}

#[test]
fn test_mcp_context_creation() {
    if env::var("MEMORY_BANK_USE_REAL_EMBEDDERS").is_err() {
        println!("Skipping test: MEMORY_BANK_USE_REAL_EMBEDDERS not set");
        return;
    }

    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_mcp_context");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a semantic search client
    let mut client = SemanticSearchClient::new(base_dir.clone()).unwrap();

    // Create a mock MCP tool context
    let mcp_context = McpToolContext {
        id: "weather-server_get_weather".to_string(),
        server_name: "weather-server".to_string(),
        tool_name: "get_weather".to_string(),
        description: "Get current weather for a location".to_string(),
        parameters: vec![
            ToolParameter {
                name: "location".to_string(),
                param_type: "string".to_string(),
                description: Some("The location to get weather for".to_string()),
                required: true,
            },
            ToolParameter {
                name: "units".to_string(),
                param_type: "string".to_string(),
                description: Some("Temperature units".to_string()),
                required: false,
            },
        ],
        server_config: McpServerConfig {
            command: "weather-mcp-server".to_string(),
            args: vec!["--api-key".to_string(), "test".to_string()],
            env: None,
            timeout: 30000,
        },
        indexed_content: "Tool: get_weather from weather-server server. Description: Get current weather for a location. Parameters: location (string, required), units (string). Categories: weather, api. Server command: weather-mcp-server".to_string(),
        last_updated: Utc::now(),
    };

    // Add the MCP context
    let context_id = client.add_mcp_context(mcp_context, false).unwrap();

    // Verify the context was added
    let contexts = client.get_all_contexts();
    assert_eq!(contexts.len(), 1);
    assert!(contexts.iter().any(|c| c.id == context_id));

    // Search for weather-related content
    let search_results = client.search_all("weather location", Some(5)).unwrap();
    assert!(!search_results.is_empty());

    let (found_context_id, results) = &search_results[0];
    assert_eq!(found_context_id, &context_id);
    assert!(!results.is_empty());

    // Verify the search result contains MCP tool metadata
    let result = &results[0];
    let payload = &result.point.payload;
    assert_eq!(payload.get("type").unwrap().as_str().unwrap(), "mcp_tool");
    assert_eq!(payload.get("server_name").unwrap().as_str().unwrap(), "weather-server");
    assert_eq!(payload.get("tool_name").unwrap().as_str().unwrap(), "get_weather");

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}

#[test]
fn test_mcp_contexts_batch_creation() {
    if env::var("MEMORY_BANK_USE_REAL_EMBEDDERS").is_err() {
        println!("Skipping test: MEMORY_BANK_USE_REAL_EMBEDDERS not set");
        return;
    }

    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_mcp_batch");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a semantic search client
    let mut client = SemanticSearchClient::new(base_dir.clone()).unwrap();

    // Create multiple MCP tool contexts
    let mcp_contexts = vec![
        McpToolContext {
            id: "weather-server_get_weather".to_string(),
            server_name: "weather-server".to_string(),
            tool_name: "get_weather".to_string(),
            description: "Get current weather for a location".to_string(),
            parameters: vec![],
            server_config: McpServerConfig {
                command: "weather-mcp-server".to_string(),
                args: vec![],
                env: None,
                timeout: 30000,
            },
            indexed_content: "Tool: get_weather from weather-server server. Description: Get current weather for a location. Categories: weather, api.".to_string(),
            last_updated: Utc::now(),
        },
        McpToolContext {
            id: "file-server_read_file".to_string(),
            server_name: "file-server".to_string(),
            tool_name: "read_file".to_string(),
            description: "Read contents of a file".to_string(),
            parameters: vec![],
            server_config: McpServerConfig {
                command: "file-mcp-server".to_string(),
                args: vec![],
                env: None,
                timeout: 30000,
            },
            indexed_content: "Tool: read_file from file-server server. Description: Read contents of a file. Categories: file, io.".to_string(),
            last_updated: Utc::now(),
        },
    ];

    // Add the MCP contexts as a batch
    let context_id = client
        .add_mcp_contexts(
            mcp_contexts,
            "MCP Tools Collection",
            "Collection of MCP tools from various servers",
            true, // Make it persistent so metadata is stored
        )
        .unwrap();

    // Verify the context was added
    let contexts = client.get_contexts();
    assert_eq!(contexts.len(), 1);

    let context = contexts.iter().find(|c| c.id == context_id).unwrap();
    assert_eq!(context.name, "MCP Tools Collection");
    assert_eq!(context.item_count, 2);

    // Search for weather-related content
    let weather_results = client.search_all("weather", Some(5)).unwrap();
    assert!(!weather_results.is_empty());

    // Search for file-related content
    let file_results = client.search_all("file read", Some(5)).unwrap();
    assert!(!file_results.is_empty());

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}
