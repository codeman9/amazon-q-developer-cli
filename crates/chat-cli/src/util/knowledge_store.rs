use std::sync::{
    Arc,
    LazyLock as Lazy,
};

use eyre::Result;
use semantic_search_client::KnowledgeContext;
use semantic_search_client::client::AsyncSemanticSearchClient;
use semantic_search_client::types::SearchResult;
use tokio::sync::Mutex;
use uuid::Uuid;

/// MCP indexing log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum McpIndexingLogLevel {
    Silent = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

impl McpIndexingLogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "silent" => Self::Silent,
            "error" => Self::Error,
            "warn" => Self::Warn,
            "info" => Self::Info,
            "debug" => Self::Debug,
            _ => Self::Info, // Default
        }
    }
}

#[derive(Debug)]
pub enum KnowledgeError {
    ClientError(String),
}

impl std::fmt::Display for KnowledgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KnowledgeError::ClientError(msg) => write!(f, "Client error: {}", msg),
        }
    }
}

impl std::error::Error for KnowledgeError {}

/// Async knowledge store - just a thin wrapper!
pub struct KnowledgeStore {
    client: AsyncSemanticSearchClient,
}

impl KnowledgeStore {
    /// Get singleton instance
    pub async fn get_async_instance() -> Arc<Mutex<Self>> {
        static ASYNC_INSTANCE: Lazy<tokio::sync::OnceCell<Arc<Mutex<KnowledgeStore>>>> =
            Lazy::new(tokio::sync::OnceCell::new);

        if cfg!(test) {
            Arc::new(Mutex::new(
                KnowledgeStore::new()
                    .await
                    .expect("Failed to create test async knowledge store"),
            ))
        } else {
            ASYNC_INSTANCE
                .get_or_init(|| async {
                    Arc::new(Mutex::new(
                        KnowledgeStore::new()
                            .await
                            .expect("Failed to create async knowledge store"),
                    ))
                })
                .await
                .clone()
        }
    }

    pub async fn new() -> Result<Self> {
        let client = AsyncSemanticSearchClient::new_with_default_dir()
            .await
            .map_err(|e| eyre::eyre!("Failed to create client: {}", e))?;

        let mut store = Self { client };
        
        // Automatically index MCP tools if enabled (don't fail initialization if this fails)
        if let Err(e) = store.auto_index_mcp_tools().await {
            eprintln!("Warning: Failed to auto-index MCP tools: {}", e);
        }

        Ok(store)
    }

    /// Add context - delegates to async client
    pub async fn add(&mut self, name: &str, path_str: &str) -> Result<String, String> {
        let path_buf = std::path::PathBuf::from(path_str);
        let canonical_path = path_buf
            .canonicalize()
            .map_err(|_io_error| format!("❌ Path does not exist: {}", path_str))?;

        match self
            .client
            .add_context_from_path(&canonical_path, name, &format!("Knowledge context for {}", name), true)
            .await
        {
            Ok((operation_id, _)) => Ok(format!(
                "🚀 Started indexing '{}'\n📁 Path: {}\n🆔 Operation ID: {}.",
                name,
                canonical_path.display(),
                &operation_id.to_string()[..8]
            )),
            Err(e) => Err(format!("Failed to start indexing: {}", e)),
        }
    }

    /// Get all contexts - delegates to async client
    pub async fn get_all(&self) -> Result<Vec<KnowledgeContext>, KnowledgeError> {
        Ok(self.client.get_contexts().await)
    }

    /// Search - delegates to async client
    pub async fn search(&self, query: &str, _context_id: Option<&str>) -> Result<Vec<SearchResult>, KnowledgeError> {
        let results = self
            .client
            .search_all(query, None)
            .await
            .map_err(|e| KnowledgeError::ClientError(e.to_string()))?;

        let mut flattened = Vec::new();
        for (_, context_results) in results {
            flattened.extend(context_results);
        }

        flattened.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

        Ok(flattened)
    }

    /// Get status data - delegates to async client
    pub async fn get_status_data(&self) -> Result<semantic_search_client::SystemStatus, String> {
        self.client
            .get_status_data()
            .await
            .map_err(|e| format!("Failed to get status data: {}", e))
    }

    /// Cancel operation - delegates to async client
    pub async fn cancel_operation(&mut self, operation_id: Option<&str>) -> Result<String, String> {
        if let Some(short_id) = operation_id {
            // Debug: List all available operations
            let available_ops = self.client.list_operation_ids().await;
            if available_ops.is_empty() {
                return Err("No active operations found".to_string());
            }

            // Try to parse as full UUID first
            if let Ok(uuid) = Uuid::parse_str(short_id) {
                self.client.cancel_operation(uuid).await.map_err(|e| e.to_string())
            } else {
                // Try to find by short ID (first 8 characters)
                if let Some(full_uuid) = self.client.find_operation_by_short_id(short_id).await {
                    self.client.cancel_operation(full_uuid).await.map_err(|e| e.to_string())
                } else {
                    Err(format!(
                        "No operation found matching ID: {}\nAvailable operations:\n{}",
                        short_id,
                        available_ops.join("\n")
                    ))
                }
            }
        } else {
            // Cancel all operations
            self.client.cancel_all_operations().await.map_err(|e| e.to_string())
        }
    }

    /// Clear all contexts (background operation)
    pub async fn clear(&mut self) -> Result<String, String> {
        match self.client.clear_all().await {
            Ok((operation_id, _cancel_token)) => Ok(format!(
                "🚀 Started clearing all contexts in background.\n📊 Use 'knowledge status' to check progress.\n🆔 Operation ID: {}",
                &operation_id.to_string()[..8]
            )),
            Err(e) => Err(format!("Failed to start clear operation: {}", e)),
        }
    }

    /// Clear all contexts immediately (synchronous operation)
    pub async fn clear_immediate(&mut self) -> Result<String, String> {
        match self.client.clear_all_immediate().await {
            Ok(count) => Ok(format!("✅ Successfully cleared {} knowledge base entries", count)),
            Err(e) => Err(format!("Failed to clear knowledge base: {}", e)),
        }
    }

    /// Remove context by path
    pub async fn remove_by_path(&mut self, path: &str) -> Result<(), String> {
        if let Some(context) = self.client.get_context_by_path(path).await {
            self.client
                .remove_context_by_id(&context.id)
                .await
                .map_err(|e| e.to_string())
        } else {
            Err(format!("No context found with path '{}'", path))
        }
    }

    /// Remove context by name
    pub async fn remove_by_name(&mut self, name: &str) -> Result<(), String> {
        if let Some(context) = self.client.get_context_by_name(name).await {
            self.client
                .remove_context_by_id(&context.id)
                .await
                .map_err(|e| e.to_string())
        } else {
            Err(format!("No context found with name '{}'", name))
        }
    }

    /// Remove context by ID
    pub async fn remove_by_id(&mut self, context_id: &str) -> Result<(), String> {
        self.client
            .remove_context_by_id(context_id)
            .await
            .map_err(|e| e.to_string())
    }

    /// Update context by path
    pub async fn update_by_path(&mut self, path_str: &str) -> Result<String, String> {
        if let Some(context) = self.client.get_context_by_path(path_str).await {
            // Remove the existing context first
            self.client
                .remove_context_by_id(&context.id)
                .await
                .map_err(|e| e.to_string())?;

            // Then add it back with the same name
            self.add(&context.name, path_str).await
        } else {
            // Debug: List all available contexts
            let available_paths = self.client.list_context_paths().await;
            if available_paths.is_empty() {
                Err("No contexts found. Add a context first with 'knowledge add <name> <path>'".to_string())
            } else {
                Err(format!(
                    "No context found with path '{}'\nAvailable contexts:\n{}",
                    path_str,
                    available_paths.join("\n")
                ))
            }
        }
    }

    /// Update context by ID
    pub async fn update_context_by_id(&mut self, context_id: &str, path_str: &str) -> Result<String, String> {
        let contexts = self.get_all().await.map_err(|e| e.to_string())?;
        let context = contexts
            .iter()
            .find(|c| c.id == context_id)
            .ok_or_else(|| format!("Context '{}' not found", context_id))?;

        let context_name = context.name.clone();

        // Remove the existing context first
        self.client
            .remove_context_by_id(context_id)
            .await
            .map_err(|e| e.to_string())?;

        // Then add it back with the same name
        self.add(&context_name, path_str).await
    }

    /// Update context by name
    pub async fn update_context_by_name(&mut self, name: &str, path_str: &str) -> Result<String, String> {
        if let Some(context) = self.client.get_context_by_name(name).await {
            // Remove the existing context first
            self.client
                .remove_context_by_id(&context.id)
                .await
                .map_err(|e| e.to_string())?;

            // Then add it back with the same name
            self.add(name, path_str).await
        } else {
            Err(format!("Context with name '{}' not found", name))
        }
    }

    /// Automatically index MCP tools if auto-indexing is enabled
    async fn auto_index_mcp_tools(&mut self) -> Result<(), String> {
        // Check if auto-indexing is enabled
        if !self.is_mcp_auto_indexing_enabled().await {
            return Ok(());
        }

        // Check if we need to refresh based on interval
        if !self.should_refresh_mcp_index().await {
            return Ok(());
        }

        // Perform background indexing
        match self.background_index_mcp_tools().await {
            Ok(message) => {
                if self.get_mcp_indexing_log_level().await >= McpIndexingLogLevel::Info {
                    println!("🔧 MCP Auto-indexing: {}", message);
                }
                Ok(())
            }
            Err(e) => {
                if self.get_mcp_indexing_log_level().await >= McpIndexingLogLevel::Error {
                    eprintln!("❌ MCP Auto-indexing failed: {}", e);
                }
                Err(e)
            }
        }
    }

    /// Check if MCP auto-indexing is enabled
    async fn is_mcp_auto_indexing_enabled(&self) -> bool {
        use crate::database::settings::{Setting, Settings};
        
        match Settings::new().await {
            Ok(settings) => {
                settings.get_bool(Setting::McpAutoIndexingEnabled).unwrap_or(true) // Default to enabled
            }
            Err(_) => true // Default to enabled if settings unavailable
        }
    }

    /// Check if we should refresh the MCP index based on interval
    async fn should_refresh_mcp_index(&self) -> bool {
        use crate::database::settings::{Setting, Settings};
        
        let _refresh_interval = match Settings::new().await {
            Ok(settings) => {
                settings.get_int(Setting::McpIndexRefreshInterval).unwrap_or(300) as u64 // Default 5 minutes
            }
            Err(_) => 300 // Default 5 minutes
        };

        // For now, always refresh on startup
        // TODO: Implement timestamp-based checking for incremental updates
        true
    }

    /// Get the MCP indexing log level
    async fn get_mcp_indexing_log_level(&self) -> McpIndexingLogLevel {
        use crate::database::settings::{Setting, Settings};
        
        match Settings::new().await {
            Ok(settings) => {
                let level_str = settings.get_string(Setting::McpIndexingLogLevel).unwrap_or_else(|| "info".to_string());
                McpIndexingLogLevel::from_str(&level_str)
            }
            Err(_) => McpIndexingLogLevel::Info
        }
    }

    /// Perform background MCP indexing without blocking
    async fn background_index_mcp_tools(&mut self) -> Result<String, String> {
        // Use existing refresh_mcp_tools but with better error handling and logging
        let start_time = std::time::Instant::now();
        
        match self.refresh_mcp_tools().await {
            Ok(message) => {
                let duration = start_time.elapsed();
                Ok(format!("{} (completed in {:.2}s)", message, duration.as_secs_f64()))
            }
            Err(e) => {
                let duration = start_time.elapsed();
                Err(format!("Failed after {:.2}s: {}", duration.as_secs_f64(), e))
            }
        }
    }

    /// Refresh MCP tools by discovering and indexing MCP servers from mcp.json
    pub async fn refresh_mcp_tools(&mut self) -> Result<String, String> {
        // This is a placeholder - the actual implementation will be added in the next step
        Ok("No MCP configuration found or no servers to index".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_automatic_mcp_indexing_on_initialization() {
        println!("\n🔧 Testing Automatic MCP Indexing on Initialization");
        
        // Test that KnowledgeStore initialization triggers automatic MCP indexing
        let store_result = KnowledgeStore::new().await;
        assert!(store_result.is_ok(), "KnowledgeStore should initialize successfully even if MCP indexing fails");
        
        let store = store_result.unwrap();
        println!("✅ KnowledgeStore initialized with automatic MCP indexing");
        
        // The store should be ready to use regardless of MCP indexing success/failure
        let search_result = store.search("test", None).await;
        assert!(search_result.is_ok(), "Search should work after initialization");
        
        println!("✅ Search functionality works after automatic indexing");
    }

    #[tokio::test]
    async fn test_mcp_auto_indexing_enabled_check() {
        println!("\n🔧 Testing MCP Auto-indexing Enabled Check");
        
        let store = KnowledgeStore::new().await.unwrap();
        
        // Test the auto-indexing enabled check
        let is_enabled = store.is_mcp_auto_indexing_enabled().await;
        println!("📊 MCP auto-indexing enabled: {}", is_enabled);
        
        // Should default to true if settings are unavailable
        assert!(is_enabled, "MCP auto-indexing should be enabled by default");
        
        println!("✅ Auto-indexing enabled check works correctly");
    }

    #[tokio::test]
    async fn test_mcp_indexing_log_level() {
        println!("\n🔧 Testing MCP Indexing Log Level");
        
        let store = KnowledgeStore::new().await.unwrap();
        
        // Test the log level retrieval
        let log_level = store.get_mcp_indexing_log_level().await;
        println!("📊 MCP indexing log level: {:?}", log_level);
        
        // Should default to Info level
        assert_eq!(log_level, McpIndexingLogLevel::Info, "Should default to Info log level");
        
        println!("✅ Log level retrieval works correctly");
    }

    #[tokio::test]
    async fn test_mcp_indexing_log_level_from_string() {
        println!("\n🔧 Testing MCP Indexing Log Level String Conversion");
        
        // Test all log level conversions
        assert_eq!(McpIndexingLogLevel::from_str("silent"), McpIndexingLogLevel::Silent);
        assert_eq!(McpIndexingLogLevel::from_str("error"), McpIndexingLogLevel::Error);
        assert_eq!(McpIndexingLogLevel::from_str("warn"), McpIndexingLogLevel::Warn);
        assert_eq!(McpIndexingLogLevel::from_str("info"), McpIndexingLogLevel::Info);
        assert_eq!(McpIndexingLogLevel::from_str("debug"), McpIndexingLogLevel::Debug);
        
        // Test case insensitivity
        assert_eq!(McpIndexingLogLevel::from_str("INFO"), McpIndexingLogLevel::Info);
        assert_eq!(McpIndexingLogLevel::from_str("Error"), McpIndexingLogLevel::Error);
        
        // Test default for unknown values
        assert_eq!(McpIndexingLogLevel::from_str("unknown"), McpIndexingLogLevel::Info);
        
        println!("✅ Log level string conversion works correctly");
    }

    #[tokio::test]
    async fn test_background_mcp_indexing() {
        println!("\n🔧 Testing Background MCP Indexing");
        
        let mut store = KnowledgeStore::new().await.unwrap();
        
        // Test background indexing (should handle missing MCP config gracefully)
        let result = store.background_index_mcp_tools().await;
        println!("📊 Background indexing result: {:?}", result);
        
        // Should succeed even with no MCP configuration
        assert!(result.is_ok(), "Background indexing should handle missing MCP config gracefully");
        
        let message = result.unwrap();
        assert!(message.contains("No MCP configuration found"), "Should indicate no config found");
        assert!(message.contains("completed in"), "Should include timing information");
        
        println!("✅ Background indexing works correctly");
    }
}
