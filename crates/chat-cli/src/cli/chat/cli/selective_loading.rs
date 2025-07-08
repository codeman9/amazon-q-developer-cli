use std::io::Write;

use clap::{
    Parser,
    Subcommand,
};
use eyre::Result;

use crate::cli::chat::{
    ChatSession,
    ChatState,
};
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
}

impl SelectiveLoadingSubcommand {
    pub async fn execute(self, _os: &mut Os, session: &mut ChatSession) -> Result<ChatState> {
        use crate::database::settings::{
            Setting,
            Settings,
        };

        let result = match self {
            SelectiveLoadingSubcommand::Status => {
                let settings = Settings::new().await?;
                let enabled = settings.get_bool(Setting::McpSelectiveLoadingEnabled).unwrap_or(false);

                if enabled {
                    "🔧 Selective MCP Loading: ✅ ENABLED\n💡 Servers are loaded on-demand based on your queries"
                        .to_string()
                } else {
                    "🔧 Selective MCP Loading: ❌ DISABLED (all servers loaded at startup)".to_string()
                }
            },
            SelectiveLoadingSubcommand::Enable => {
                let mut settings = Settings::new().await?;
                settings.set(Setting::McpSelectiveLoadingEnabled, true).await?;
                "✅ Selective MCP Loading enabled! MCP servers will now be loaded on-demand based on your queries.\n💡 Restart your chat session for changes to take effect.".to_string()
            },
            SelectiveLoadingSubcommand::Disable => {
                let mut settings = Settings::new().await?;
                settings.set(Setting::McpSelectiveLoadingEnabled, false).await?;
                "✅ Selective MCP Loading disabled! All enabled MCP servers will be loaded at startup.\n💡 Restart your chat session for changes to take effect.".to_string()
            },
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
            },
        };

        // Print the result
        writeln!(session.stdout, "{}", result)?;

        Ok(ChatState::PromptUser {
            skip_printing_tools: false,
        })
    }
}
