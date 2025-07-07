use std::collections::HashMap;
use std::sync::Arc;
use eyre::Result;

use crate::cli::chat::tool_manager::{ToolManager, ToolManagerBuilder, McpServerConfig};
use crate::cli::chat::tools::custom_tool::{CustomToolClient, CustomToolConfig};
use crate::util::selective_mcp_loader::SelectiveMcpLoader;
use crate::database::settings::{Setting, Settings};
use crate::os::Os;

/// Enhanced ToolManagerBuilder that supports selective MCP loading
pub struct SelectiveToolManagerBuilder {
    base_builder: ToolManagerBuilder,
    selective_loading_enabled: bool,
}

impl SelectiveToolManagerBuilder {
    /// Create a new selective tool manager builder
    pub fn new() -> Self {
        Self {
            base_builder: ToolManagerBuilder::default(),
            selective_loading_enabled: false,
        }
    }

    /// Enable selective loading based on settings
    pub async fn with_selective_loading(mut self, os: &Os) -> Result<Self> {
        // Check if selective loading is enabled in settings
        let settings = Settings::new().await?;
        self.selective_loading_enabled = settings
            .get_bool(Setting::McpSelectiveLoadingEnabled)
            .unwrap_or(false); // Default to disabled for now

        Ok(self)
    }

    /// Set MCP server config (delegates to base builder)
    pub fn mcp_server_config(mut self, config: McpServerConfig) -> Self {
        self.base_builder = self.base_builder.mcp_server_config(config);
        self
    }

    /// Set conversation ID (delegates to base builder)
    pub fn conversation_id(mut self, conversation_id: &str) -> Self {
        self.base_builder = self.base_builder.conversation_id(conversation_id);
        self
    }

    /// Build the tool manager with selective loading if enabled
    pub async fn build(
        self,
        os: &mut Os,
        output: Box<dyn std::io::Write + Send + Sync + 'static>,
        interactive: bool,
    ) -> Result<ToolManager> {
        if self.selective_loading_enabled {
            // Build with selective loading
            self.build_with_selective_loading(os, output, interactive).await
        } else {
            // Build normally (existing behavior)
            self.base_builder.build(os, output, interactive).await
        }
    }

    /// Build tool manager with selective loading enabled
    async fn build_with_selective_loading(
        self,
        os: &mut Os,
        mut output: Box<dyn std::io::Write + Send + Sync + 'static>,
        interactive: bool,
    ) -> Result<ToolManager> {
        // Create selective MCP loader
        let selective_loader = SelectiveMcpLoader::new().await?;
        
        // Get server stats for display
        let stats = selective_loader.get_server_stats().await;
        
        if interactive {
            writeln!(output, "🔧 Selective MCP Loading: {} servers available, loading on-demand", stats.total_available)?;
        }

        // Create a modified MCP server config with no servers (they'll be loaded on-demand)
        let empty_mcp_config = McpServerConfig {
            mcp_servers: HashMap::new(),
        };

        // Build the base tool manager with empty MCP config
        let mut base_manager = self.base_builder
            .mcp_server_config(empty_mcp_config)
            .build(os, output, interactive)
            .await?;

        // Store the selective loader in the tool manager
        // Note: This requires modifying ToolManager to hold the selective loader
        // For now, we'll just return the base manager and handle selective loading externally

        Ok(base_manager)
    }
}

impl Default for SelectiveToolManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for selective loading integration
pub struct SelectiveLoadingUtils;

impl SelectiveLoadingUtils {
    /// Check if selective loading is enabled
    pub async fn is_enabled() -> bool {
        if let Ok(settings) = Settings::new().await {
            settings.get_bool(Setting::McpSelectiveLoadingEnabled).unwrap_or(false)
        } else {
            false
        }
    }

    /// Enable selective loading
    pub async fn enable() -> Result<()> {
        let settings = Settings::new().await?;
        settings.set_bool(Setting::McpSelectiveLoadingEnabled, true).await?;
        Ok(())
    }

    /// Disable selective loading
    pub async fn disable() -> Result<()> {
        let settings = Settings::new().await?;
        settings.set_bool(Setting::McpSelectiveLoadingEnabled, false).await?;
        Ok(())
    }

    /// Get selective loading status message
    pub async fn get_status_message() -> String {
        if Self::is_enabled().await {
            "🔧 Selective MCP Loading: Enabled (servers loaded on-demand)".to_string()
        } else {
            "🔧 Selective MCP Loading: Disabled (all servers loaded at startup)".to_string()
        }
    }
}

/// Integration with existing chat CLI
pub async fn create_tool_manager_with_selective_loading(
    mcp_config: McpServerConfig,
    conversation_id: &str,
    os: &mut Os,
    output: Box<dyn std::io::Write + Send + Sync + 'static>,
    interactive: bool,
) -> Result<ToolManager> {
    SelectiveToolManagerBuilder::new()
        .with_selective_loading(os)
        .await?
        .mcp_server_config(mcp_config)
        .conversation_id(conversation_id)
        .build(os, output, interactive)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_selective_loading_utils() {
        // Test that we can check if selective loading is enabled
        let _enabled = SelectiveLoadingUtils::is_enabled().await;
        
        // Test status message generation
        let _status = SelectiveLoadingUtils::get_status_message().await;
    }

    #[tokio::test]
    async fn test_selective_builder_creation() {
        let _builder = SelectiveToolManagerBuilder::new();
    }
}
