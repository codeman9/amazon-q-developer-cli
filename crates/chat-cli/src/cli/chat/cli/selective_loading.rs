use std::io::Write;
use clap::{Parser, Subcommand};
use eyre::Result;

use crate::cli::chat::{ChatState, ChatSession};
use crate::os::Os;

#[derive(Debug, PartialEq, Parser)]
pub struct SelectiveLoadingArgs;

#[derive(Debug, PartialEq, Subcommand)]
pub enum SelectiveLoadingSubcommand {
    /// Show selective loading status
    Status,
    /// Enable selective MCP loading
    Enable,
    /// Disable selective MCP loading
    Disable,
    /// Toggle selective MCP loading
    Toggle,
    /// Show help for selective loading
    Help,
}

impl SelectiveLoadingSubcommand {
    pub async fn execute(self, _os: &mut Os, session: &mut ChatSession) -> Result<ChatState> {
        use crate::database::settings::{Setting, Settings};
        
        let result = match self {
            SelectiveLoadingSubcommand::Status => {
                let settings = Settings::new().await?;
                let enabled = settings.get_bool(Setting::McpSelectiveLoadingEnabled).unwrap_or(false);
                
                if enabled {
                    "🔧 Selective MCP Loading: ✅ ENABLED\n💡 Servers are loaded on-demand based on your queries".to_string()
                } else {
                    "🔧 Selective MCP Loading: ❌ DISABLED (all servers loaded at startup)".to_string()
                }
            }
            SelectiveLoadingSubcommand::Enable => {
                let mut settings = Settings::new().await?;
                settings.set(Setting::McpSelectiveLoadingEnabled, true).await?;
                "✅ Selective MCP Loading enabled! MCP servers will now be loaded on-demand based on your queries.\n💡 Restart your chat session for changes to take effect.".to_string()
            }
            SelectiveLoadingSubcommand::Disable => {
                let mut settings = Settings::new().await?;
                settings.set(Setting::McpSelectiveLoadingEnabled, false).await?;
                "✅ Selective MCP Loading disabled! All enabled MCP servers will be loaded at startup.\n💡 Restart your chat session for changes to take effect.".to_string()
            }
            SelectiveLoadingSubcommand::Toggle => {
                let mut settings = Settings::new().await?;
                let currently_enabled = settings.get_bool(Setting::McpSelectiveLoadingEnabled).unwrap_or(false);
                
                if currently_enabled {
                    settings.set(Setting::McpSelectiveLoadingEnabled, false).await?;
                    "✅ Selective MCP Loading disabled! All enabled MCP servers will be loaded at startup.\n💡 Restart your chat session for changes to take effect.".to_string()
                } else {
                    settings.set(Setting::McpSelectiveLoadingEnabled, true).await?;
                    "✅ Selective MCP Loading enabled! MCP servers will now be loaded on-demand based on your queries.\n💡 Restart your chat session for changes to take effect.".to_string()
                }
            }
            SelectiveLoadingSubcommand::Help => {
                "Selective MCP Loading Commands:
        
🔧 /selective-loading status    - Show current status
🔧 /selective-loading enable    - Enable selective loading
🔧 /selective-loading disable   - Disable selective loading  
🔧 /selective-loading toggle    - Toggle selective loading

What is Selective Loading?
• Instead of loading ALL MCP servers at startup
• Only loads servers that contain tools relevant to your queries
• Reduces memory usage and startup time
• Provides 50%+ token reduction in conversations
• Uses RAG (semantic search) to find the right tools

Benefits:
✅ Faster startup (only essential servers loaded)
✅ Lower memory usage (unused servers not loaded)
✅ Better performance (fewer active connections)
✅ Smarter tool selection (RAG-based relevance)
✅ Same functionality (tools loaded when needed)

Note: Changes require restarting your chat session to take effect.".to_string()
            }
        };

        // Print the result
        writeln!(session.stdout, "{}", result)?;
        
        Ok(ChatState::PromptUser { skip_printing_tools: false })
    }
}
