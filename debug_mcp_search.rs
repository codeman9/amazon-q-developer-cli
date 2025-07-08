use chat_cli::util::knowledge_store::KnowledgeStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Debugging MCP search...");

    let knowledge_store = KnowledgeStore::new().await?;

    // Test direct search
    println!("1. Testing direct search...");
    let search_results = knowledge_store.search("tool", None).await?;
    println!("   Found {} search results", search_results.len());

    for result in search_results.iter().take(3) {
        println!("   - Distance: {}", result.distance);
        println!(
            "     Payload keys: {:?}",
            result.point.payload.keys().collect::<Vec<_>>()
        );
        if let Some(content) = result.point.payload.get("content") {
            println!(
                "     Content: {}",
                content.as_str().unwrap_or("").chars().take(100).collect::<String>()
            );
        }
        if let Some(tool_name) = result.point.payload.get("tool_name") {
            println!("     Tool name: {}", tool_name);
        }
        if let Some(server_name) = result.point.payload.get("server_name") {
            println!("     Server name: {}", server_name);
        }
    }

    // Test MCP-specific search
    println!("2. Testing MCP search method...");
    match knowledge_store.search_mcp_tools_for_llm("tool", Some(10)).await {
        Ok(tools) => {
            println!("   Found {} MCP tools via search method", tools.len());
            for tool in tools.iter().take(3) {
                println!("   - Tool: {:?}", tool);
            }
        },
        Err(e) => println!("   Error: {}", e),
    }

    Ok(())
}
