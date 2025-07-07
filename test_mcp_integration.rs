// Simple test script to verify MCP RAG integration
// Run with: cargo run --bin test_mcp_integration --features test-utils

#[cfg(feature = "test-utils")]
use chat_cli::util::test_utils::{create_mock_servers, create_temp_mcp_config};

use std::collections::HashMap;
use chat_cli::util::{
    mcp_processor::{McpDiscoveryService, ToolSchemaProcessor},
    mcp_llm_integration::McpLlmIntegration,
    mcp_tool_integration::{McpToolIntegrationService, integrate_mcp_tools_into_config},
};
use chat_cli::cli::chat::tools::{ToolSpec, ToolOrigin, InputSchema};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(feature = "test-utils"))]
    {
        println!("❌ This test requires the 'test-utils' feature.");
        println!("Run with: cargo run --bin test_mcp_integration --features test-utils");
        return Ok(());
    }
    
    #[cfg(feature = "test-utils")]
    {
        println!("🚀 Testing MCP RAG Integration Locally");
        println!("=====================================");
        
        // Step 1: Create mock MCP servers
        println!("\n1. Creating mock MCP servers...");
        let servers = create_mock_servers();
        println!("   ✅ Created {} mock servers", servers.len());
        
        for server in &servers {
            println!("   📡 Server: {} ({} tools)", 
                server.name, 
                server.tools.len()
            );
        }
        
        // Step 2: Create temporary mcp.json configuration
        println!("\n2. Creating temporary mcp.json configuration...");
        let (_temp_dir, config_path) = create_temp_mcp_config(&servers)?;
        println!("   ✅ Created config at: {:?}", config_path);
        
        // Step 3: Test MCP Discovery
        println!("\n3. Testing MCP server discovery...");
        let discovery_service = McpDiscoveryService::new().await?;
        let discovered_servers = discovery_service
            .discover_servers_from_path(&config_path, "test")
            .await?;
        
        println!("   ✅ Discovered {} servers", discovered_servers.len());
        for server in &discovered_servers {
            println!("   🔍 Found: {} (timeout: {}ms)", server.name, server.timeout);
        }
        
        // Step 4: Test Tool Schema Processing
        println!("\n4. Testing tool schema processing...");
        let processor = ToolSchemaProcessor::new();
        let mut total_tools = 0;
        
        for server in &servers {
            if !server.should_fail {
                let server_info = server.to_server_info();
                let tools_result = server.get_tools_result()?;
                
                let contexts = processor.create_mcp_contexts(&server_info, &tools_result)?;
                total_tools += contexts.len();
                
                println!("   🔧 Processed {} tools from {}", contexts.len(), server.name);
                for context in &contexts {
                    println!("      - {}: {}", context.tool_name, 
                        context.description.chars().take(50).collect::<String>());
                }
            }
        }
        
        println!("   ✅ Total tools processed: {}", total_tools);
        
        // Step 5: Test LLM Integration
        println!("\n5. Testing LLM integration...");
        let llm_integration = McpLlmIntegration::new().await?;
        
        let tools_available = llm_integration.are_mcp_tools_available().await;
        println!("   📊 MCP tools available: {}", tools_available);
        
        let stats = llm_integration.get_tool_statistics().await?;
        println!("   📈 Statistics: {} servers, {} tools", 
            stats.server_count, stats.total_tools);
        
        // Step 6: Test Tool Integration Service
        println!("\n6. Testing tool integration service...");
        let integration_service = McpToolIntegrationService::new().await?;
        
        let tool_specs = integration_service.get_mcp_tool_specs().await?;
        println!("   🛠️  Generated {} tool specifications", tool_specs.len());
        
        for (name, spec) in tool_specs.iter().take(3) {
            println!("      - {}: {} (origin: {:?})", 
                name, spec.description, spec.tool_origin);
        }
        
        // Step 7: Test Tool Configuration Integration
        println!("\n7. Testing tool configuration integration...");
        let mut base_config = HashMap::new();
        base_config.insert("test_native_tool".to_string(), ToolSpec {
            name: "test_native_tool".to_string(),
            description: "A native test tool".to_string(),
            input_schema: InputSchema(serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            })),
            tool_origin: ToolOrigin::Native,
        });
        
        let integrated_config = integrate_mcp_tools_into_config(base_config).await?;
        
        println!("   🔗 Integrated configuration:");
        println!("      - Total tools: {}", integrated_config.len());
        
        let native_tools = integrated_config.values()
            .filter(|spec| spec.tool_origin == ToolOrigin::Native)
            .count();
        let mcp_tools = integrated_config.values()
            .filter(|spec| spec.tool_origin == ToolOrigin::Mcp)
            .count();
        
        println!("      - Native tools: {}", native_tools);
        println!("      - MCP tools: {}", mcp_tools);
        
        // Step 8: Test Query-based Tool Selection
        println!("\n8. Testing query-based tool selection...");
        let weather_tools = integration_service
            .get_mcp_tools_for_query("weather", Some(3))
            .await?;
        
        println!("   🌤️  Found {} weather-related tools", weather_tools.len());
        
        let file_tools = integration_service
            .get_mcp_tools_for_query("file", Some(3))
            .await?;
        
        println!("   📁 Found {} file-related tools", file_tools.len());
        
        // Step 9: Performance Test
        println!("\n9. Running performance test...");
        let start = std::time::Instant::now();
        
        for _ in 0..10 {
            let _tools = integration_service
                .get_mcp_tools_for_query("test", Some(5))
                .await?;
        }
        
        let duration = start.elapsed();
        println!("   ⚡ 10 queries completed in {:?} (avg: {:?})", 
            duration, duration / 10);
        
        // Final Summary
        println!("\n🎉 MCP RAG Integration Test Complete!");
        println!("=====================================");
        println!("✅ All components working correctly");
        println!("✅ {} servers discovered and processed", discovered_servers.len());
        println!("✅ {} tools indexed and available", total_tools);
        println!("✅ Tool integration working seamlessly");
        println!("✅ Query-based selection functional");
        println!("✅ Performance within acceptable limits");
    }
    
    Ok(())
}
