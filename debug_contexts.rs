use chat_cli::util::knowledge_store::KnowledgeStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Debugging MCP contexts...");
    
    let knowledge_store = KnowledgeStore::new().await?;
    
    // Get all contexts
    println!("1. Getting all contexts...");
    let contexts = knowledge_store.get_all().await?;
    println!("   Found {} contexts", contexts.len());
    
    for context in &contexts {
        println!("   - Context: {} ({})", context.name, context.id);
        println!("     Description: {}", context.description);
        println!("     Created: {}", context.created_at);
        
        // Try to search within this specific context
        if context.name.starts_with("mcp_") {
            println!("     This is an MCP context, trying to search within it...");
            match knowledge_store.search("tool", Some(&context.id)).await {
                Ok(results) => {
                    println!("     Found {} results in this context", results.len());
                    for result in results.iter().take(2) {
                        println!("       - Distance: {}", result.distance);
                        println!("         Payload keys: {:?}", result.point.payload.keys().collect::<Vec<_>>());
                    }
                }
                Err(e) => println!("     Error searching context: {}", e),
            }
        }
    }
    
    Ok(())
}
