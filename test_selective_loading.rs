use chat_cli::util::selective_mcp_loader::SelectiveMcpLoader;
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔧 Testing Selective MCP Loading System");
    
    // Create selective loader
    match SelectiveMcpLoader::new().await {
        Ok(loader) => {
            println!("✅ Selective MCP Loader created successfully");
            
            // Get server statistics
            let stats = loader.get_server_stats().await;
            println!("📊 Server Statistics:");
            println!("   - Total available servers: {}", stats.total_available);
            println!("   - Currently loaded servers: {}", stats.currently_loaded);
            println!("   - Available servers: {:?}", stats.available_servers);
            
            // Test query-based server selection
            println!("\n🔍 Testing query-based server selection:");
            
            let test_queries = vec![
                "commit my changes to git",
                "fetch data from a URL", 
                "help me think through this step by step",
                "build my Xcode project",
            ];
            
            for query in test_queries {
                match loader.get_servers_for_query(query, Some(3)).await {
                    Ok(servers) => {
                        println!("   Query: '{}' -> Servers: {:?}", query, servers);
                    }
                    Err(e) => {
                        println!("   Query: '{}' -> Error: {}", query, e);
                    }
                }
            }
            
            // Test smart loading
            println!("\n🧠 Testing smart loading:");
            match loader.smart_load(Some("I need to commit my code changes")).await {
                Ok(loaded_servers) => {
                    println!("   Smart load successful! Loaded {} servers", loaded_servers.len());
                    for (name, _) in loaded_servers {
                        println!("   - Loaded server: {}", name);
                    }
                }
                Err(e) => {
                    println!("   Smart load failed: {}", e);
                }
            }
            
            // Final stats
            let final_stats = loader.get_server_stats().await;
            println!("\n📊 Final Statistics:");
            println!("   - Currently loaded servers: {}", final_stats.currently_loaded);
            println!("   - Loaded servers: {:?}", final_stats.loaded_servers);
            
        }
        Err(e) => {
            println!("❌ Failed to create Selective MCP Loader: {}", e);
            println!("💡 This might be because:");
            println!("   - No MCP configuration found");
            println!("   - MCP servers are not properly configured");
            println!("   - Knowledge store is not initialized");
        }
    }
    
    println!("\n🎯 Selective Loading Test Complete!");
    Ok(())
}
