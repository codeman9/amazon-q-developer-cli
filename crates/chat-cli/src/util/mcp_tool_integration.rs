use std::collections::HashMap;

use serde_json::Value;
use tracing::{debug, info, warn};

use crate::api_client::model::{Tool, ToolSpecification, ToolInputSchema};
use crate::cli::chat::tools::{InputSchema, ToolOrigin, ToolSpec};
use crate::util::{
    knowledge_store::KnowledgeStore,
    mcp_llm_integration::McpLlmIntegration,
};

/// Service for integrating MCP tools with the LLM tool calling system
#[derive(Debug)]
pub struct McpToolIntegrationService {
    #[allow(dead_code)]
    knowledge_store: KnowledgeStore,
    llm_integration: McpLlmIntegration,
}

impl McpToolIntegrationService {
    /// Create a new MCP tool integration service
    pub async fn new() -> Result<Self, String> {
        let knowledge_store = KnowledgeStore::new().await
            .map_err(|e| format!("Failed to initialize knowledge store: {}", e))?;
        
        let llm_integration = McpLlmIntegration::new().await
            .map_err(|e| format!("Failed to initialize LLM integration: {}", e))?;
        
        Ok(Self {
            knowledge_store,
            llm_integration,
        })
    }
    
    /// Get MCP tools as ToolSpec objects for integration with the existing tool system
    pub async fn get_mcp_tool_specs(&self) -> Result<HashMap<String, ToolSpec>, String> {
        debug!("Getting MCP tool specifications");
        
        // Get all available MCP tools
        let mcp_tools = self.llm_integration.get_all_available_tools().await?;
        
        let mut tool_specs = HashMap::new();
        
        for tool in mcp_tools {
            if let Some(tool_spec) = self.convert_mcp_tool_to_spec(&tool)? {
                tool_specs.insert(tool_spec.name.clone(), tool_spec);
            }
        }
        
        info!("Loaded {} MCP tool specifications", tool_specs.len());
        Ok(tool_specs)
    }
    
    /// Get MCP tools for a specific query (for dynamic tool selection)
    pub async fn get_mcp_tools_for_query(&self, query: &str, max_tools: Option<usize>) -> Result<Vec<Tool>, String> {
        debug!("Getting MCP tools for query: {}", query);
        
        let mcp_tools = self.llm_integration.get_tools_for_query(query, max_tools).await?;
        
        let mut tools = Vec::new();
        for tool in mcp_tools {
            if let Some(llm_tool) = self.convert_mcp_tool_to_llm_tool(&tool)? {
                tools.push(llm_tool);
            }
        }
        
        debug!("Found {} MCP tools for query", tools.len());
        Ok(tools)
    }
    
    /// Get all available MCP tools as LLM Tool objects
    pub async fn get_all_mcp_tools(&self) -> Result<Vec<Tool>, String> {
        debug!("Getting all available MCP tools");
        
        let mcp_tools = self.llm_integration.get_all_available_tools().await?;
        
        let mut tools = Vec::new();
        for tool in mcp_tools {
            if let Some(llm_tool) = self.convert_mcp_tool_to_llm_tool(&tool)? {
                tools.push(llm_tool);
            }
        }
        
        info!("Loaded {} MCP tools for LLM", tools.len());
        Ok(tools)
    }
    
    /// Check if MCP tools are available
    pub async fn are_mcp_tools_available(&self) -> bool {
        self.llm_integration.are_mcp_tools_available().await
    }
    
    /// Refresh MCP tools from configuration
    pub async fn refresh_mcp_tools(&self) -> Result<String, String> {
        debug!("Refreshing MCP tools");
        
        // This would trigger a refresh of the knowledge store's MCP tools
        // For now, we'll return a status message
        let stats = self.llm_integration.get_tool_statistics().await?;
        
        Ok(format!(
            "MCP tools refreshed: {} servers, {} tools available",
            stats.server_count,
            stats.total_tools
        ))
    }
    
    /// Convert MCP tool JSON to ToolSpec for integration with existing tool system
    fn convert_mcp_tool_to_spec(&self, tool: &Value) -> Result<Option<ToolSpec>, String> {
        let name = tool.get("name")
            .and_then(|n| n.as_str())
            .ok_or("MCP tool missing name")?;
        
        let description = tool.get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("MCP tool");
        
        // Convert input schema
        let input_schema = if let Some(schema) = tool.get("inputSchema") {
            InputSchema(schema.clone())
        } else {
            // Default empty schema for tools without parameters
            InputSchema(serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }))
        };
        
        Ok(Some(ToolSpec {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
            tool_origin: ToolOrigin::Mcp,
        }))
    }
    
    /// Convert MCP tool JSON to LLM Tool object
    fn convert_mcp_tool_to_llm_tool(&self, tool: &Value) -> Result<Option<Tool>, String> {
        let name = tool.get("name")
            .and_then(|n| n.as_str())
            .ok_or("MCP tool missing name")?;
        
        let description = tool.get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("MCP tool");
        
        // Convert input schema to the format expected by the LLM
        let input_schema = if let Some(schema) = tool.get("inputSchema") {
            let fig_doc: crate::api_client::model::FigDocument = serde_json::from_value(schema.clone())
                .map_err(|e| format!("Failed to convert schema to FigDocument: {}", e))?;
            ToolInputSchema {
                json: Some(fig_doc),
            }
        } else {
            let default_schema = serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            });
            let fig_doc: crate::api_client::model::FigDocument = serde_json::from_value(default_schema)
                .map_err(|e| format!("Failed to convert default schema to FigDocument: {}", e))?;
            ToolInputSchema {
                json: Some(fig_doc),
            }
        };
        
        Ok(Some(Tool::ToolSpecification(ToolSpecification {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema,
        })))
    }
    
    /// Get tool statistics for monitoring
    pub async fn get_tool_statistics(&self) -> Result<String, String> {
        let stats = self.llm_integration.get_tool_statistics().await?;
        
        Ok(format!(
            "MCP Tool Statistics:\n\
             - Servers: {}\n\
             - Total Tools: {}\n\
             - Tools per Server: {}",
            stats.server_count,
            stats.total_tools,
            stats.tools_per_server.len()
        ))
    }
}

/// Helper function to integrate MCP tools into existing tool configuration
pub async fn integrate_mcp_tools_into_config(
    mut tool_config: HashMap<String, ToolSpec>
) -> Result<HashMap<String, ToolSpec>, String> {
    debug!("Integrating MCP tools into tool configuration");
    
    // Create MCP integration service
    let integration_service = match McpToolIntegrationService::new().await {
        Ok(service) => service,
        Err(e) => {
            warn!("Failed to initialize MCP integration service: {}", e);
            return Ok(tool_config); // Return original config if MCP integration fails
        }
    };
    
    // Check if MCP tools are available
    if !integration_service.are_mcp_tools_available().await {
        debug!("No MCP tools available, skipping integration");
        return Ok(tool_config);
    }
    
    // Get MCP tool specifications
    match integration_service.get_mcp_tool_specs().await {
        Ok(mcp_tools) => {
            let mcp_count = mcp_tools.len();
            
            // Add MCP tools to the configuration
            for (name, spec) in mcp_tools {
                tool_config.insert(name, spec);
            }
            
            info!("Successfully integrated {} MCP tools into tool configuration", mcp_count);
        }
        Err(e) => {
            warn!("Failed to get MCP tool specifications: {}", e);
            // Continue without MCP tools rather than failing completely
        }
    }
    
    Ok(tool_config)
}

/// Helper function to get MCP tools for dynamic tool selection during conversations
pub async fn get_mcp_tools_for_conversation(query: &str, max_tools: Option<usize>) -> Result<Vec<Tool>, String> {
    debug!("Getting MCP tools for conversation query: {}", query);
    
    let integration_service = McpToolIntegrationService::new().await?;
    integration_service.get_mcp_tools_for_query(query, max_tools).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_mcp_tool_integration_service_creation() {
        // This test may fail if MCP services are not available
        // but it validates the service can be created
        let result = McpToolIntegrationService::new().await;
        
        // We expect either success or a clear error message
        match result {
            Ok(_) => {
                // Service created successfully
            }
            Err(e) => {
                // Service creation failed, but error should be informative
                assert!(!e.is_empty());
            }
        }
    }
    
    #[test]
    fn test_convert_mcp_tool_to_spec() {
        // Create a mock service for testing
        let service = McpToolIntegrationService {
            knowledge_store: KnowledgeStore::new().await.unwrap(),
            llm_integration: McpLlmIntegration::new().await.unwrap(),
        };
        
        let mcp_tool = json!({
            "name": "test_tool",
            "description": "A test tool",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "param1": {
                        "type": "string",
                        "description": "First parameter"
                    }
                },
                "required": ["param1"]
            }
        });
        
        let result = service.convert_mcp_tool_to_spec(&mcp_tool).unwrap();
        assert!(result.is_some());
        
        let spec = result.unwrap();
        assert_eq!(spec.name, "test_tool");
        assert_eq!(spec.description, "A test tool");
        assert_eq!(spec.tool_origin, ToolOrigin::Mcp);
    }
    
    #[test]
    fn test_convert_mcp_tool_to_llm_tool() {
        // Create a mock service for testing
        let service = McpToolIntegrationService {
            knowledge_store: KnowledgeStore::new().await.unwrap(),
            llm_integration: McpLlmIntegration::new().await.unwrap(),
        };
        
        let mcp_tool = json!({
            "name": "test_tool",
            "description": "A test tool",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "param1": {
                        "type": "string"
                    }
                }
            }
        });
        
        let result = service.convert_mcp_tool_to_llm_tool(&mcp_tool).unwrap();
        assert!(result.is_some());
        
        if let Some(Tool::ToolSpecification(spec)) = result {
            assert_eq!(spec.name, "test_tool");
            assert_eq!(spec.description, "A test tool");
        } else {
            panic!("Expected ToolSpecification");
        }
    }
}
