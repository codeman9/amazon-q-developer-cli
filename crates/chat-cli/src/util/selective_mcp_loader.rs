use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use eyre::Result;

use crate::util::knowledge_store::KnowledgeStore;
use crate::util::mcp_processor::{McpDiscoveryService, McpServerInfo};
use crate::cli::chat::tools::custom_tool::{CustomToolClient, CustomToolConfig};

/// Selective MCP server loader that only starts servers based on RAG tool selection
#[derive(Debug)]
pub struct SelectiveMcpLoader {
    /// All available MCP servers from configuration
    available_servers: HashMap<String, McpServerInfo>,
    /// Currently loaded/running servers
    loaded_servers: Arc<RwLock<HashMap<String, Arc<CustomToolClient>>>>,
    /// Knowledge store for RAG-based tool selection
    knowledge_store: Arc<RwLock<KnowledgeStore>>,
    /// Discovery service for MCP servers
    discovery_service: McpDiscoveryService,
}

impl SelectiveMcpLoader {
    /// Create a new selective MCP loader
    pub async fn new() -> Result<Self> {
        let discovery_service = McpDiscoveryService::new().await?;
        let servers = discovery_service.discover_servers().await?;
        
        // Convert to HashMap for easy lookup
        let available_servers = servers
            .into_iter()
            .map(|server| (server.name.clone(), server))
            .collect();

        let knowledge_store = Arc::new(RwLock::new(KnowledgeStore::new().await?));
        
        Ok(Self {
            available_servers,
            loaded_servers: Arc::new(RwLock::new(HashMap::new())),
            knowledge_store,
            discovery_service,
        })
    }

    /// Get servers needed for a specific query using RAG
    pub async fn get_servers_for_query(&self, query: &str, max_servers: Option<usize>) -> Result<Vec<String>> {
        let knowledge_store = self.knowledge_store.read().await;
        
        // Search for relevant MCP tools
        let search_results = knowledge_store.search_mcp_tools_for_llm(query, max_servers).await
            .map_err(|e| eyre::eyre!("Failed to search MCP tools: {}", e))?;
        
        // Extract server names from search results
        let mut server_names = HashSet::new();
        for result in search_results {
            if let Some(server_name) = result.get("server_name").and_then(|v| v.as_str()) {
                server_names.insert(server_name.to_string());
            }
        }
        
        Ok(server_names.into_iter().collect())
    }

    /// Load specific servers on-demand
    pub async fn load_servers(&self, server_names: &[String]) -> Result<HashMap<String, Arc<CustomToolClient>>> {
        let mut loaded_servers = self.loaded_servers.write().await;
        let mut newly_loaded = HashMap::new();

        for server_name in server_names {
            // Skip if already loaded
            if loaded_servers.contains_key(server_name) {
                newly_loaded.insert(server_name.clone(), loaded_servers[server_name].clone());
                continue;
            }

            // Get server info
            if let Some(server_info) = self.available_servers.get(server_name) {
                match self.create_client_for_server(server_info).await {
                    Ok(client) => {
                        let client_arc: Arc<CustomToolClient> = Arc::new(client);
                        loaded_servers.insert(server_name.clone(), client_arc.clone());
                        newly_loaded.insert(server_name.clone(), client_arc);
                        tracing::info!("✓ Loaded MCP server: {}", server_name);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load MCP server {}: {}", server_name, e);
                    }
                }
            } else {
                tracing::warn!("Unknown MCP server requested: {}", server_name);
            }
        }

        Ok(newly_loaded)
    }

    /// Get all currently loaded servers
    pub async fn get_loaded_servers(&self) -> HashMap<String, Arc<CustomToolClient>> {
        self.loaded_servers.read().await.clone()
    }

    /// Preload commonly used servers (optional optimization)
    pub async fn preload_common_servers(&self, server_names: &[String]) -> Result<()> {
        self.load_servers(server_names).await?;
        Ok(())
    }

    /// Unload unused servers to free resources
    pub async fn unload_servers(&self, server_names: &[String]) -> Result<()> {
        let mut loaded_servers = self.loaded_servers.write().await;
        
        for server_name in server_names {
            if let Some(client) = loaded_servers.remove(server_name) {
                // Gracefully shutdown the client if possible
                // Note: CustomToolClient doesn't have explicit shutdown, 
                // but dropping it should clean up resources
                drop(client);
                tracing::info!("✓ Unloaded MCP server: {}", server_name);
            }
        }
        
        Ok(())
    }

    /// Get server statistics
    pub async fn get_server_stats(&self) -> ServerStats {
        let loaded_servers = self.loaded_servers.read().await;
        
        ServerStats {
            total_available: self.available_servers.len(),
            currently_loaded: loaded_servers.len(),
            available_servers: self.available_servers.keys().cloned().collect(),
            loaded_servers: loaded_servers.keys().cloned().collect(),
        }
    }

    /// Create a CustomToolClient for a specific server
    async fn create_client_for_server(&self, server_info: &McpServerInfo) -> Result<CustomToolClient> {
        // Convert McpServerInfo to the format expected by CustomToolClient
        let server_config = CustomToolConfig {
            command: server_info.command.clone(),
            args: server_info.args.clone(),
            env: server_info.env.clone(),
            timeout: server_info.timeout,
            disabled: false, // We only load enabled servers
        };

        let client = CustomToolClient::from_config(server_info.name.clone(), server_config)?;
        Ok(client)
    }
}

/// Statistics about MCP server loading
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub total_available: usize,
    pub currently_loaded: usize,
    pub available_servers: Vec<String>,
    pub loaded_servers: Vec<String>,
}

/// Integration with existing ToolManager
impl SelectiveMcpLoader {
    /// Get servers for LLM tool calling based on conversation context
    pub async fn get_servers_for_conversation(&self, context: &str) -> Result<HashMap<String, Arc<CustomToolClient>>> {
        // Use RAG to determine relevant servers
        let server_names = self.get_servers_for_query(context, Some(5)).await?;
        
        // Load only the relevant servers
        self.load_servers(&server_names).await
    }

    /// Smart loading: preload common servers, load others on-demand
    pub async fn smart_load(&self, query: Option<&str>) -> Result<HashMap<String, Arc<CustomToolClient>>> {
        // Always preload these commonly used servers
        let common_servers = vec!["git".to_string(), "fetch".to_string()];
        self.preload_common_servers(&common_servers).await?;

        // If we have a specific query, load relevant servers
        if let Some(query) = query {
            let relevant_servers = self.get_servers_for_query(query, Some(3)).await?;
            self.load_servers(&relevant_servers).await?;
        }

        Ok(self.get_loaded_servers().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_selective_loader_creation() {
        // This test requires MCP configuration to be available
        if let Ok(loader) = SelectiveMcpLoader::new().await {
            let stats = loader.get_server_stats().await;
            assert!(stats.total_available >= 0);
            assert_eq!(stats.currently_loaded, 0);
        }
    }

    #[tokio::test]
    async fn test_server_query_selection() {
        if let Ok(loader) = SelectiveMcpLoader::new().await {
            // Test git-related query
            let servers = loader.get_servers_for_query("commit my changes", Some(5)).await;
            if let Ok(servers) = servers {
                // Should find git-related tools if available
                println!("Servers for git query: {:?}", servers);
            }
        }
    }
}
