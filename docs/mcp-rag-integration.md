# MCP RAG Tool Integration

## Overview

The Amazon Q CLI now includes intelligent MCP (Model Context Protocol) tool selection using Retrieval-Augmented Generation (RAG). This feature automatically discovers MCP tools from your configuration and makes them available to the LLM through semantic search, solving the "prompt bloat" problem when working with large numbers of MCP servers.

## Key Benefits

- **Automatic Discovery**: MCP tools are automatically discovered from your `mcp.json` configuration
- **Intelligent Selection**: RAG-based semantic search finds the most relevant tools for each query
- **Seamless Integration**: Tools appear in LLM conversations without additional commands
- **Scalable**: Handles hundreds of MCP servers without performance degradation
- **Token Efficient**: Reduces prompt size by 50%+ compared to including all tools

## How It Works

### 1. Automatic Tool Discovery
When you start a chat session, the system automatically:
- Reads your `mcp.json` configuration file
- Discovers enabled MCP servers
- Indexes tool schemas and descriptions
- Creates searchable embeddings for semantic matching

### 2. Intelligent Tool Selection
During conversations, the system:
- Analyzes your queries semantically
- Searches the tool index for relevant matches
- Provides only the most relevant tools to the LLM
- Maintains high accuracy while reducing token usage

### 3. Seamless Tool Execution
When the LLM selects MCP tools:
- Tools are executed automatically with your permission
- Results are integrated into the conversation
- Error handling is consistent with existing tools

## Configuration

### Basic Setup

The MCP RAG integration works with your existing `mcp.json` configuration:

```json
{
  "servers": {
    "weather-server": {
      "command": "weather-mcp-server",
      "args": ["--api-key", "your-api-key"],
      "timeout": 30000
    },
    "file-server": {
      "command": "file-mcp-server",
      "args": ["--safe-mode"],
      "timeout": 15000
    },
    "database-server": {
      "command": "db-mcp-server",
      "args": ["--connection", "sqlite:///data.db"],
      "timeout": 45000
    }
  }
}
```

### Advanced Configuration

You can control MCP RAG behavior through settings:

```bash
# Enable/disable MCP tool indexing (default: enabled)
q config set mcp.autoIndexing.enabled true

# Set refresh interval in seconds (default: 300)
q config set mcp.autoIndexing.refreshInterval 600

# Set logging level for MCP operations (default: info)
q config set mcp.autoIndexing.logLevel debug
```

## Usage Examples

### Weather Queries
```
You: What's the weather like in San Francisco?

# The system automatically finds and uses weather-related MCP tools
Assistant: I'll check the current weather in San Francisco for you.
[Uses weather-server/get_current_weather tool automatically]
```

### File Operations
```
You: Can you read the contents of my config file?

# The system finds file-related tools and asks for the specific path
Assistant: I can help you read a file. Which config file would you like me to read?
You: ./config/app.json
[Uses file-server/read_file tool automatically]
```

### Database Queries
```
You: Show me the users table from the database

# The system finds database tools and executes the query
Assistant: I'll query the users table for you.
[Uses database-server/query_table tool automatically]
```

## Tool Management

### Viewing Available Tools
```bash
# List all available tools (including MCP tools)
q chat --tools

# View MCP-specific statistics
q knowledge status
```

### Refreshing MCP Tools
```bash
# Manually refresh MCP tool index
q knowledge refresh-mcp

# Clear and rebuild the entire knowledge base
q knowledge clear
```

### Tool Permissions
```bash
# Trust all MCP tools automatically
q chat --trust-all-tools

# Trust specific MCP tools
q chat --trust-tools=weather-server_get_weather,file-server_read_file
```

## Performance Characteristics

### Expected Performance
- **Tool Discovery**: 30-60 seconds for initial indexing (one-time)
- **Search Response**: <100ms for tool selection
- **Memory Usage**: ~50-100MB additional for tool index
- **Storage**: ~10-50MB for tool metadata

### Optimization Tips
1. **Limit Server Count**: Keep to 10-50 MCP servers for optimal performance
2. **Use Descriptive Names**: Clear tool names and descriptions improve matching
3. **Regular Refresh**: Refresh the index when adding new MCP servers
4. **Monitor Performance**: Use debug logging to monitor indexing performance

## Troubleshooting

### Common Issues

#### No MCP Tools Available
```
Warning: No MCP tools available, skipping integration
```
**Solution**: Check your `mcp.json` configuration and ensure MCP servers are enabled.

#### Tool Discovery Fails
```
Failed to initialize MCP integration service: Connection failed
```
**Solution**: Verify MCP servers are running and accessible.

#### Slow Performance
```
MCP tool indexing taking longer than expected
```
**Solution**: Reduce the number of MCP servers or increase refresh interval.

### Debug Information

Enable debug logging to troubleshoot issues:
```bash
q config set mcp.autoIndexing.logLevel debug
q chat
```

This will show detailed information about:
- MCP server discovery
- Tool schema processing
- Semantic search operations
- Error conditions and recovery

### Log Locations

MCP RAG logs are integrated with the main Q CLI logging:
- **macOS**: `~/Library/Logs/Amazon Q/`
- **Linux**: `~/.local/share/amazon-q/logs/`
- **Windows**: `%APPDATA%/Amazon Q/logs/`

## Advanced Features

### Custom Tool Categories
You can organize tools by adding categories to your MCP server descriptions:
```json
{
  "servers": {
    "weather-server": {
      "command": "weather-mcp-server",
      "description": "Weather and climate data tools",
      "categories": ["weather", "api", "data"]
    }
  }
}
```

### Tool Usage Analytics
The system tracks tool usage for better recommendations:
- Frequently used tools are prioritized
- Usage patterns improve semantic matching
- Statistics are available through the knowledge command

### Integration with Knowledge Base
MCP tools work seamlessly with the existing knowledge system:
- Search across both documents and tools
- Unified results in knowledge queries
- Consistent permission and trust model

## Best Practices

### Tool Organization
1. **Use Clear Names**: Tool names should be descriptive and unique
2. **Add Rich Descriptions**: Detailed descriptions improve semantic matching
3. **Group Related Tools**: Keep related tools on the same MCP server
4. **Document Parameters**: Clear parameter descriptions help with tool selection

### Performance Optimization
1. **Limit Tool Count**: Keep individual servers to <50 tools for best performance
2. **Use Specific Queries**: More specific queries get better tool matches
3. **Regular Maintenance**: Refresh the index when adding/removing tools
4. **Monitor Usage**: Use statistics to identify unused tools

### Security Considerations
1. **Review Tool Permissions**: Understand what each MCP tool can do
2. **Use Trust Settings**: Configure tool trust levels appropriately
3. **Monitor Tool Usage**: Review tool execution logs regularly
4. **Keep Tools Updated**: Ensure MCP servers are running current versions

## API Reference

### Configuration Settings
- `mcp.autoIndexing.enabled`: Enable/disable automatic MCP tool indexing
- `mcp.autoIndexing.refreshInterval`: How often to refresh the tool index (seconds)
- `mcp.autoIndexing.logLevel`: Logging level for MCP operations

### Commands
- `q knowledge refresh-mcp`: Manually refresh MCP tool index
- `q knowledge status`: Show MCP tool statistics
- `q chat --tools`: List all available tools including MCP tools

### Environment Variables
- `MCP_CONFIG_PATH`: Override default mcp.json location
- `MCP_INDEXING_DISABLED`: Disable MCP tool indexing entirely
- `MCP_LOG_LEVEL`: Override MCP logging level

## Migration Guide

### From Manual MCP Tool Management
If you were previously managing MCP tools manually:

1. **Remove Manual Configurations**: The RAG system will auto-discover tools
2. **Update Trust Settings**: Review and update tool trust configurations
3. **Test Tool Access**: Verify all needed tools are still accessible
4. **Monitor Performance**: Check that tool selection performance meets your needs

### Upgrading Existing Setups
When upgrading to the RAG-enabled version:

1. **Backup Configuration**: Save your current `mcp.json` configuration
2. **Test in Development**: Try the new system in a test environment first
3. **Gradual Migration**: Enable RAG for a subset of tools initially
4. **Monitor and Adjust**: Use debug logging to fine-tune the configuration

## Support and Feedback

For issues, questions, or feedback about MCP RAG integration:

1. **Check Logs**: Enable debug logging for detailed troubleshooting information
2. **Review Documentation**: This guide covers most common scenarios
3. **Community Support**: Join the Amazon Q CLI community discussions
4. **Report Issues**: Use the built-in issue reporting for bugs or feature requests

The MCP RAG integration represents a significant improvement in tool management and selection, making it easier to work with large numbers of MCP servers while maintaining high performance and accuracy.
