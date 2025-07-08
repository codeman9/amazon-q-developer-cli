use std::collections::HashMap;

use chat_cli::util::knowledge_store::KnowledgeStore;
use chat_cli::util::mcp_tool_integration::{
    McpToolIntegrationService,
    integrate_mcp_tools_into_config,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing MCP integration...");

    // Test 1: Check if knowledge store has MCP contexts
    println!("1. Testing KnowledgeStore...");
    let knowledge_store = KnowledgeStore::new().await?;
    let contexts = knowledge_store.get_all().await?;
    println!("   Found {} contexts", contexts.len());
    for context in &contexts {
        if context.name.starts_with("mcp_") {
            println!("   - MCP Context: {}", context.name);
        }
    }

    // Test 2: Try to get MCP tools for LLM with different queries
    println!("2. Testing MCP tools for LLM...");
    let queries = ["tool", "build", "xcode", "function", ""];
    for query in &queries {
        if query.is_empty() {
            continue; // Skip empty query as it causes errors
        }
        println!("   Trying query: '{}'", query);
        match knowledge_store.search_mcp_tools_for_llm(query, Some(10)).await {
            Ok(tools) => {
                println!("   Found {} MCP tools for query '{}'", tools.len(), query);
                if tools.len() > 0 {
                    break; // Found some tools, stop trying
                }
            },
            Err(e) => println!("   Error getting MCP tools for '{}': {}", query, e),
        }
    }

    // Test 3: Try MCP integration service
    println!("3. Testing MCP integration service...");
    match McpToolIntegrationService::new().await {
        Ok(service) => {
            println!("   MCP integration service created successfully");
            match service.get_mcp_tool_specs().await {
                Ok(specs) => println!("   Got {} tool specs", specs.len()),
                Err(e) => println!("   Error getting tool specs: {}", e),
            }
        },
        Err(e) => println!("   Error creating MCP integration service: {}", e),
    }

    // Test 4: Try integration function
    println!("4. Testing integration function...");
    let empty_config = HashMap::new();
    match integrate_mcp_tools_into_config(empty_config).await {
        Ok(integrated) => println!("   Integration successful, got {} tools", integrated.len()),
        Err(e) => println!("   Integration failed: {}", e),
    }

    Ok(())
}
