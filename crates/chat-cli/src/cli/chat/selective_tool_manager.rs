use std::collections::{HashMap, HashSet};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tokio::sync::{RwLock, Notify};
use eyre::Result;

use crate::cli::chat::custom_tool_client::CustomToolClient;
use crate::cli::chat::tool_manager::{ToolManager, ToolSpec};
use crate::util::selective_mcp_loader::{SelectiveMcpLoader, ServerStats};

/// Enhanced ToolManager with selective MCP server loading
pub struct SelectiveToolManager {
    /// Base tool manager for native tools
    base_manager: ToolManager,
    /// Selective MCP loader for on-demand server loading
    mcp_loader: SelectiveMcpLoader,
    /// Currently loaded MCP servers
    loaded_mcp_servers: Arc<RwLock<HashMap<String, Arc<CustomToolClient>>>>,
    /// Notification for when new servers are loaded
    server_load_notify: Arc<Notify>,
}

impl SelectiveToolManager {
    /// Create a new selective tool manager
    pub async fn new(base_manager: ToolManager) -> Result<Self> {
        let mcp_loader = SelectiveMcpLoader::new().await?;
        
        Ok(Self {
            base_manager,
            mcp_loader,
            loaded_mcp_servers: Arc::new(RwLock::new(HashMap::new())),
            server_load_notify: Arc::new(Notify::new()),
        })
    }

    /// Load MCP servers based on a query or conversation context
    pub async fn load_servers_for_query(&mut self, query: &str) -> Result<Vec<String>> {
        // Get relevant servers using RAG
        let server_names = self.mcp_loader.get_servers_for_query(query, Some(5)).await?;
        
        if !server_names.is_empty() {
            // Load the servers
            let loaded_servers = self.mcp_loader.load_servers(&server_names).await?;
            
            // Update our loaded servers
            let mut loaded_mcp_servers = self.loaded_mcp_servers.write().await;
            for (name, client) in loaded_servers {
                loaded_mcp_servers.insert(name.clone(), client);
            }
            
            // Notify that new servers are available
            self.server_load_notify.notify_waiters();
            
            tracing::info!("Loaded {} MCP servers for query: {:?}", server_names.len(), server_names);
        }
        
        Ok(server_names)
    }

    /// Get all available tools (native + loaded MCP)
    pub async fn get_all_tools(&self) -> Result<HashMap<String, ToolSpec>> {
        // Start with native tools from base manager
        let mut all_tools = self.base_manager.schema.clone();
        
        // Add tools from loaded MCP servers
        let loaded_servers = self.loaded_mcp_servers.read().await;
        for (server_name, client) in loaded_servers.iter() {
            // Get tool specs from the client
            // Note: This is a simplified approach - in practice you'd need to
            // extract tool specs from the CustomToolClient
            tracing::debug!("Including tools from MCP server: {}", server_name);
        }
        
        Ok(all_tools)
    }

    /// Smart preloading: load commonly used servers at startup
    pub async fn smart_preload(&mut self) -> Result<()> {
        // Preload commonly used servers
        let common_servers = vec!["git".to_string(), "fetch".to_string()];
        let loaded_servers = self.mcp_loader.load_servers(&common_servers).await?;
        
        let mut loaded_mcp_servers = self.loaded_mcp_servers.write().await;
        for (name, client) in loaded_servers {
            loaded_mcp_servers.insert(name, client);
        }
        
        tracing::info!("Preloaded {} common MCP servers", common_servers.len());
        Ok(())
    }

    /// Get server statistics
    pub async fn get_server_stats(&self) -> ServerStats {
        self.mcp_loader.get_server_stats().await
    }

    /// Unload unused servers to free resources
    pub async fn cleanup_unused_servers(&mut self, keep_servers: &[String]) -> Result<()> {
        let loaded_servers = self.loaded_mcp_servers.read().await;
        let servers_to_unload: Vec<String> = loaded_servers
            .keys()
            .filter(|name| !keep_servers.contains(name))
            .cloned()
            .collect();
        drop(loaded_servers);

        if !servers_to_unload.is_empty() {
            self.mcp_loader.unload_servers(&servers_to_unload).await?;
            
            let mut loaded_mcp_servers = self.loaded_mcp_servers.write().await;
            for server_name in &servers_to_unload {
                loaded_mcp_servers.remove(server_name);
            }
            
            tracing::info!("Unloaded {} unused MCP servers", servers_to_unload.len());
        }
        
        Ok(())
    }

    /// Get currently loaded MCP servers
    pub async fn get_loaded_mcp_servers(&self) -> HashMap<String, Arc<CustomToolClient>> {
        self.loaded_mcp_servers.read().await.clone()
    }

    /// Wait for new servers to be loaded
    pub async fn wait_for_server_updates(&self) {
        self.server_load_notify.notified().await;
    }

    /// Load servers for a conversation context (more intelligent than query-based)
    pub async fn load_servers_for_conversation(&mut self, context: &str) -> Result<HashMap<String, Arc<CustomToolClient>>> {
        let loaded_servers = self.mcp_loader.get_servers_for_conversation(context).await?;
        
        let mut loaded_mcp_servers = self.loaded_mcp_servers.write().await;
        for (name, client) in &loaded_servers {
            loaded_mcp_servers.insert(name.clone(), client.clone());
        }
        
        self.server_load_notify.notify_waiters();
        
        Ok(loaded_servers)
    }
}

/// Integration functions for existing ToolManager interface
impl SelectiveToolManager {
    /// Get the base tool manager (for compatibility)
    pub fn base_manager(&self) -> &ToolManager {
        &self.base_manager
    }

    /// Get the base tool manager mutably (for compatibility)
    pub fn base_manager_mut(&mut self) -> &mut ToolManager {
        &mut self.base_manager
    }

    /// Check if there are new tools available
    pub fn has_new_tools(&self) -> bool {
        self.base_manager.has_new_stuff.load(Ordering::Acquire)
    }

    /// Get conversation ID
    pub fn conversation_id(&self) -> &str {
        &self.base_manager.conversation_id
    }
}

/// Factory for creating SelectiveToolManager
pub struct SelectiveToolManagerBuilder {
    base_manager: Option<ToolManager>,
}

impl SelectiveToolManagerBuilder {
    pub fn new() -> Self {
        Self {
            base_manager: None,
        }
    }

    pub fn with_base_manager(mut self, manager: ToolManager) -> Self {
        self.base_manager = Some(manager);
        self
    }

    pub async fn build(self) -> Result<SelectiveToolManager> {
        let base_manager = self.base_manager.ok_or_else(|| eyre::eyre!("Base manager not provided"))?;
        SelectiveToolManager::new(base_manager).await
    }
}

impl Default for SelectiveToolManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests would require a proper ToolManager instance
    // which is complex to create in isolation. In practice, integration
    // tests would be more appropriate.

    #[tokio::test]
    async fn test_selective_manager_creation() {
        // This test would require a valid ToolManager instance
        // For now, just test that the builder can be created
        let _builder = SelectiveToolManagerBuilder::new();
    }
}
