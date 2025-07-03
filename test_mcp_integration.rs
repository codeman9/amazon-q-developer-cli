use std::path::Path;
use chat_cli::util::knowledge_store::KnowledgeStore;
use chat_cli::util::mcp_processor::McpDiscoveryService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing MCP Integration");
    println!("========================");

    // Test 1: Create KnowledgeStore
    println!("\n1. Creating KnowledgeStore...");
    let mut store = KnowledgeStore::new().await?;
    println!("✅ KnowledgeStore created successfully");

    // Test 2: Test MCP discovery without configuration
    println!("\n2. Testing MCP discovery without configuration...");
    let result = store.refresh_mcp_tools().await;
    match result {
        Ok(message) => println!("✅ {}", message),
        Err(e) => println!("❌ Error: {}", e),
    }

    // Test 3: Test MCP discovery service directly
    println!("\n3. Testing MCP discovery service...");
    let discovery_service = McpDiscoveryService::new().await?;
    
    let has_config = discovery_service.has_mcp_configuration().await;
    println!("📁 Has MCP configuration: {}", has_config);

    if has_config {
        let servers = discovery_service.discover_servers().await?;
        println!("🔍 Found {} MCP servers:", servers.len());
        for server in &servers {
            println!("  - {} (disabled: {})", server.name, server.disabled);
        }
    }

    // Test 4: Get MCP servers from KnowledgeStore
    println!("\n4. Getting indexed MCP servers...");
    let servers = store.get_mcp_servers().await?;
    println!("📊 Indexed MCP servers: {}", servers.len());
    for server in servers {
        println!("  - {} ({} tools)", server.name, server.tool_count);
    }

    // Test 5: Test search functionality
    println!("\n5. Testing search functionality...");
    let search_results = store.search_with_mcp("weather", None).await?;
    println!("🔍 Search results for 'weather': {} results", search_results.len());

    // Test 6: Test regular search for comparison
    let regular_results = store.search("weather", None).await?;
    println!("🔍 Regular search results for 'weather': {} results", regular_results.len());

    println!("\n✅ All tests completed!");
    Ok(())
}
