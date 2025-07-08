use std::collections::HashMap;

use serde_json::Value;
use tracing::debug;

use crate::cli::chat::tools::{
    InputSchema,
    ToolOrigin,
    ToolSpec,
};
use crate::util::mcp_llm_integration::McpLlmIntegration;

/// Helper function to integrate MCP tools into existing tool configuration
pub async fn integrate_mcp_tools_into_config(
    mut tool_config: HashMap<String, ToolSpec>,
) -> Result<HashMap<String, ToolSpec>, String> {
    debug!("Integrating MCP tools into tool configuration");

    // Create MCP LLM integration service
    let llm_integration = match McpLlmIntegration::new().await {
        Ok(integration) => integration,
        Err(e) => {
            debug!("Failed to create MCP LLM integration: {}", e);
            return Ok(tool_config); // Return original config if MCP integration fails
        },
    };

    // Get available MCP tools
    let mcp_tools = match llm_integration.get_tools_for_query("", Some(50)).await {
        Ok(tools) => tools,
        Err(e) => {
            debug!("Failed to get MCP tools: {}", e);
            return Ok(tool_config); // Return original config if getting tools fails
        },
    };

    debug!("Found {} MCP tools to integrate", mcp_tools.len());

    // Convert MCP tools to ToolSpec format and add to configuration
    for tool in mcp_tools {
        if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
            if let Some(tool_spec) = convert_mcp_tool_to_tool_spec(&tool)? {
                tool_config.insert(name.to_string(), tool_spec);
            }
        }
    }

    debug!("Integrated MCP tools into configuration");
    Ok(tool_config)
}

/// Convert an MCP tool (JSON Value) to a ToolSpec
fn convert_mcp_tool_to_tool_spec(tool: &Value) -> Result<Option<ToolSpec>, String> {
    let name = tool.get("name").and_then(|n| n.as_str()).ok_or("Tool missing name")?;
    let description = tool.get("description").and_then(|d| d.as_str()).unwrap_or("");

    // Create a basic ToolSpec for the MCP tool
    let tool_spec = ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: InputSchema(
            tool.get("input_schema")
                .cloned()
                .unwrap_or(Value::Object(serde_json::Map::new())),
        ),
        tool_origin: ToolOrigin::Mcp,
    };

    Ok(Some(tool_spec))
}
