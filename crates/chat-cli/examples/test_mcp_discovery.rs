use chat_cli::util::mcp_processor::{
    McpDiscoveryService,
    ToolSchemaProcessor,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing MCP Discovery Service...");

    // Create the discovery service
    let discovery_service = McpDiscoveryService::new().await?;

    // Check if MCP configuration exists
    let has_config = discovery_service.has_mcp_configuration().await;
    println!("📁 MCP configuration exists: {}", has_config);

    if has_config {
        // Discover servers
        let servers = discovery_service.discover_servers().await?;
        println!("🖥️  Found {} MCP servers:", servers.len());

        for server in &servers {
            println!("  - {} (command: {})", server.name, server.command);
            println!("    Args: {:?}", server.args);
            println!("    Timeout: {}ms", server.timeout);
            println!("    Disabled: {}", server.disabled);
            println!();
        }

        // Test tool schema processor with mock data
        let processor = ToolSchemaProcessor::new();

        if let Some(server) = servers.first() {
            println!("🔧 Testing tool schema processing with server: {}", server.name);

            // Create mock tool schema (since we can't easily connect to real MCP servers in this example)
            let mock_tools_result = chat_cli::mcp_client::ToolsListResult {
                tools: vec![serde_json::json!({
                    "name": "example_tool",
                    "description": "An example tool for testing",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "input": {
                                "type": "string",
                                "description": "Input parameter"
                            }
                        },
                        "required": ["input"]
                    },
                    "categories": ["example", "test"]
                })],
                next_cursor: None,
            };

            // Generate searchable content
            let searchable_content = processor.extract_searchable_content(server, &mock_tools_result);
            println!("📝 Generated searchable content:");
            for content in &searchable_content {
                println!("  {}", content);
            }

            // Create MCP contexts
            let contexts = processor.create_mcp_contexts(server, &mock_tools_result)?;
            println!("\n🎯 Created {} MCP contexts:", contexts.len());
            for context in &contexts {
                println!("  - ID: {}", context.id);
                println!("    Tool: {} from {}", context.tool_name, context.server_name);
                println!("    Description: {}", context.description);
                println!("    Parameters: {} params", context.parameters.len());
                println!();
            }
        }
    } else {
        println!("❌ No MCP configuration found. Create ~/.config/amazon-q/mcp.json or ./.amazon-q/mcp.json to test.");
        println!("\nExample configuration:");
        println!(
            r#"{{
  "mcpServers": {{
    "test-server": {{
      "command": "echo",
      "args": ["Hello from MCP"],
      "timeout": 5000,
      "disabled": false
    }}
  }}
}}"#
        );
    }

    println!("✅ MCP Discovery test complete!");
    Ok(())
}
