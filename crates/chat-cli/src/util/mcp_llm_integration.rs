use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;

use crate::util::knowledge_store::KnowledgeStore;

/// Service for integrating MCP tools with LLM function calling
pub struct McpLlmIntegration {
    knowledge_store: Arc<RwLock<KnowledgeStore>>,
}

impl McpLlmIntegration {
    /// Create a new MCP LLM integration service
    pub async fn new() -> Result<Self, String> {
        let knowledge_store = Arc::new(RwLock::new(KnowledgeStore::new().await.map_err(|e| e.to_string())?));

        Ok(Self { knowledge_store })
    }

    /// Get relevant MCP tools for a user query to provide to the LLM
    pub async fn get_tools_for_query(&self, query: &str, max_tools: Option<usize>) -> Result<Vec<Value>, String> {
        let limit = max_tools.unwrap_or(10);

        // Use semantic search to find relevant MCP tools
        self.knowledge_store
            .read()
            .await
            .search_mcp_tools_for_llm(query, Some(limit))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_llm_integration_creation() {
        let integration_result = McpLlmIntegration::new().await;
        assert!(
            integration_result.is_ok(),
            "Should be able to create MCP LLM integration"
        );

        println!("✅ MCP LLM integration created successfully");
    }

    #[tokio::test]
    async fn test_tool_search_functionality() {
        let integration = McpLlmIntegration::new().await.unwrap();

        // Test searching for tools
        let tools_result = integration.get_tools_for_query("weather", Some(5)).await;
        match &tools_result {
            Ok(tools) => println!("📊 Found {} tools for 'weather' query", tools.len()),
            Err(e) => println!("❌ Error searching for tools: {}", e),
        }
        assert!(tools_result.is_ok(), "Should be able to search for tools");

        println!("✅ Tool search functionality test completed");
    }
}
