use std::collections::HashMap;

use eyre::Result;
use serde_json::Value;

use crate::util::knowledge_store::KnowledgeStore;

/// Service for integrating MCP tools with LLM function calling
#[derive(Debug)]
pub struct McpLlmIntegration {
    knowledge_store: KnowledgeStore,
}

impl McpLlmIntegration {
    /// Create a new MCP LLM integration service
    pub async fn new() -> Result<Self> {
        let knowledge_store = KnowledgeStore::new().await?;
        Ok(Self { knowledge_store })
    }

    /// Get relevant MCP tools for a user query to provide to the LLM
    pub async fn get_tools_for_query(&self, query: &str, max_tools: Option<usize>) -> Result<Vec<Value>, String> {
        let limit = max_tools.unwrap_or(10);
        
        // Use semantic search to find relevant MCP tools
        self.knowledge_store.get_relevant_mcp_tools(query, limit).await
    }

    /// Get all available MCP tools as function definitions
    pub async fn get_all_available_tools(&self) -> Result<Vec<Value>, String> {
        self.knowledge_store.get_all_mcp_tools_for_llm().await
    }

    /// Convert MCP tools to the format expected by the LLM tool system
    pub fn convert_to_tool_specs(&self, mcp_tools: Vec<Value>) -> HashMap<String, Value> {
        let mut tool_specs = HashMap::new();
        
        for tool in mcp_tools {
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                tool_specs.insert(name.to_string(), tool);
            }
        }
        
        tool_specs
    }

    /// Filter tools based on user preferences and context
    pub async fn filter_tools_for_context(&self, tools: Vec<Value>, _context: &str) -> Vec<Value> {
        // For now, return all tools
        // In a full implementation, this would filter based on:
        // - User preferences
        // - Conversation context
        // - Tool permissions
        // - Performance considerations
        tools
    }

    /// Get tool suggestions based on conversation context
    pub async fn suggest_tools_for_conversation(&self, conversation_context: &str) -> Result<Vec<Value>, String> {
        // Extract key terms from conversation context for tool search
        let key_terms = self.extract_key_terms(conversation_context);
        
        let mut all_tools = Vec::new();
        
        // Search for tools based on each key term
        for term in key_terms {
            let tools = self.get_tools_for_query(&term, Some(3)).await?;
            all_tools.extend(tools);
        }
        
        // Remove duplicates and limit results
        self.deduplicate_tools(all_tools)
    }

    /// Extract key terms from conversation context for tool search
    fn extract_key_terms(&self, context: &str) -> Vec<String> {
        // Simple keyword extraction - in a full implementation, this would be more sophisticated
        let words: Vec<String> = context
            .split_whitespace()
            .filter(|word| word.len() > 3) // Filter out short words
            .map(|word| word.to_lowercase())
            .filter(|word| !self.is_stop_word(word)) // Filter out stop words
            .take(5) // Limit to top 5 terms
            .collect();
        
        words
    }

    /// Check if a word is a stop word
    fn is_stop_word(&self, word: &str) -> bool {
        matches!(word, "the" | "and" | "or" | "but" | "in" | "on" | "at" | "to" | "for" | "of" | "with" | "by")
    }

    /// Remove duplicate tools from a list
    fn deduplicate_tools(&self, tools: Vec<Value>) -> Result<Vec<Value>, String> {
        let mut seen_names = std::collections::HashSet::new();
        let mut unique_tools = Vec::new();
        
        for tool in tools {
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                if seen_names.insert(name.to_string()) {
                    unique_tools.push(tool);
                }
            }
        }
        
        // Limit to reasonable number of tools
        unique_tools.truncate(15);
        
        Ok(unique_tools)
    }

    /// Check if MCP tools are available and indexed
    pub async fn are_mcp_tools_available(&self) -> bool {
        match self.get_all_available_tools().await {
            Ok(tools) => !tools.is_empty(),
            Err(_) => false,
        }
    }

    /// Get statistics about available MCP tools
    pub async fn get_tool_statistics(&self) -> Result<McpToolStatistics, String> {
        let tools = self.get_all_available_tools().await?;
        
        let mut server_counts = HashMap::new();
        let mut total_tools = 0;
        
        for tool in &tools {
            total_tools += 1;
            
            // Extract server name from tool name (assuming format: server_toolname)
            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                if let Some(server_name) = name.split('_').next() {
                    *server_counts.entry(server_name.to_string()).or_insert(0) += 1;
                }
            }
        }
        
        Ok(McpToolStatistics {
            total_tools,
            server_count: server_counts.len(),
            tools_per_server: server_counts,
        })
    }
}

/// Statistics about available MCP tools
#[derive(Debug, Clone)]
pub struct McpToolStatistics {
    pub total_tools: usize,
    pub server_count: usize,
    pub tools_per_server: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_llm_integration_creation() {
        
        let integration_result = McpLlmIntegration::new().await;
        assert!(integration_result.is_ok(), "Should be able to create MCP LLM integration");
        
        let integration = integration_result.unwrap();
        println!("✅ MCP LLM integration created successfully");
        
        // Test availability check
        let available = integration.are_mcp_tools_available().await;
        println!("📊 MCP tools available: {}", available);
        
        println!("✅ MCP LLM integration test completed");
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
        
        // Test getting all tools
        let all_tools_result = integration.get_all_available_tools().await;
        match &all_tools_result {
            Ok(tools) => println!("📊 Total available tools: {}", tools.len()),
            Err(e) => println!("❌ Error getting all tools: {}", e),
        }
        assert!(all_tools_result.is_ok(), "Should be able to get all tools: {:?}", all_tools_result.err());
        
        // Note: It's expected that no tools are found since we haven't indexed any MCP servers yet
        // This test verifies that the search functionality works without errors
        
        println!("✅ Tool search functionality test completed");
    }

    #[tokio::test]
    async fn test_tool_statistics() {
        
        let integration = McpLlmIntegration::new().await.unwrap();
        
        let stats_result = integration.get_tool_statistics().await;
        assert!(stats_result.is_ok(), "Should be able to get tool statistics");
        
        let stats = stats_result.unwrap();
        println!("📊 Tool Statistics:");
        println!("   Total tools: {}", stats.total_tools);
        println!("   Server count: {}", stats.server_count);
        println!("   Tools per server: {:?}", stats.tools_per_server);
        
        // Note: It's expected that stats show 0 tools since we haven't indexed any MCP servers yet
        // This test verifies that the statistics functionality works without errors
        
        println!("✅ Tool statistics test completed");
    }

    #[tokio::test]
    async fn test_key_term_extraction() {
        
        let integration = McpLlmIntegration::new().await.unwrap();
        
        let context = "I need to check the weather forecast for tomorrow and also get some file information";
        let terms = integration.extract_key_terms(context);
        
        println!("📊 Extracted terms from '{}': {:?}", context, terms);
        assert!(!terms.is_empty(), "Should extract some key terms");
        
        // Test that stop words are filtered out
        assert!(!terms.contains(&"the".to_string()), "Should filter out stop words");
        
        println!("✅ Key term extraction test completed");
    }
}
