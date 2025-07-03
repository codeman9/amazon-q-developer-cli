# Detailed Design - RAG Tool for MCP Servers

## Overview

This document presents the detailed design for extending the existing `/knowledge` command to support MCP (Model Context Protocol) server tool selection using Retrieval-Augmented Generation (RAG). The system will automatically index MCP tools from the user's `mcp.json` configuration and provide semantic search capabilities for intelligent tool selection.

## Requirements Summary

Based on the requirements clarification:

1. **Integration**: Extend existing `/knowledge` command
2. **Discovery**: Automatically read from `mcp.json` configuration file
3. **Indexing**: Full tool schemas including parameters
4. **User Interface**: Automatic integration with existing knowledge search
5. **Execution**: Integrate with existing LLM workflow for tool calling
6. **Scale**: Personal/development use (10-50 MCP servers)
7. **Error Handling**: Maintain consistency with current patterns

## Architecture

### High-Level Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   User Query    │───▶│  Knowledge Store │───▶│  LLM Tool Call  │
│                 │    │                  │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                │
                                ▼
                       ┌──────────────────┐
                       │ Semantic Search  │
                       │     Client       │
                       └──────────────────┘
                                │
                    ┌───────────┴───────────┐
                    ▼                       ▼
            ┌──────────────┐        ┌──────────────┐
            │   Document   │        │  MCP Tool    │
            │   Contexts   │        │   Contexts   │
            └──────────────┘        └──────────────┘
                                            │
                                            ▼
                                   ┌──────────────┐
                                   │  mcp.json    │
                                   │ Config File  │
                                   └──────────────┘
```

### Component Architecture

The design leverages the existing RAG infrastructure with MCP-specific extensions:

#### 1. Extended KnowledgeStore
- **Location**: `crates/chat-cli/src/util/knowledge_store.rs`
- **Enhancement**: Add MCP server discovery and indexing capabilities
- **New Methods**:
  - `discover_mcp_servers()` - Read and parse mcp.json
  - `index_mcp_tools()` - Extract and index tool schemas
  - `refresh_mcp_contexts()` - Update MCP tool index

#### 2. MCP Context Processor
- **Location**: New module `crates/chat-cli/src/util/mcp_processor.rs`
- **Purpose**: Handle MCP-specific indexing logic
- **Responsibilities**:
  - Parse mcp.json configuration
  - Connect to MCP servers and retrieve tool schemas
  - Transform tool schemas into searchable content
  - Handle MCP server health checking

#### 3. Enhanced Semantic Search Client
- **Location**: `crates/semantic_search_client/`
- **Enhancement**: Support MCP-specific context types
- **New Context Type**: `McpToolContext` alongside existing `KnowledgeContext`

## Data Models

### MCP Tool Context

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolContext {
    pub id: String,
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
    pub server_config: McpServerConfig,
    pub indexed_content: String, // Searchable text representation
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub param_type: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
    pub timeout: u64,
}
```

### Search Result Enhancement

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchResultType {
    Document,
    McpTool,
}

// Extend existing SearchResult
impl SearchResult {
    pub result_type: SearchResultType,
    pub mcp_tool_info: Option<McpToolContext>,
}
```

## Components and Interfaces

### 1. MCP Discovery Service

```rust
pub struct McpDiscoveryService {
    config_path: PathBuf,
    client_factory: McpClientFactory,
}

impl McpDiscoveryService {
    pub async fn discover_servers(&self) -> Result<Vec<McpServerInfo>, McpError>;
    pub async fn get_tool_schemas(&self, server: &McpServerInfo) -> Result<Vec<ToolSchema>, McpError>;
    pub async fn validate_server_health(&self, server: &McpServerInfo) -> Result<bool, McpError>;
}
```

### 2. Tool Schema Processor

```rust
pub struct ToolSchemaProcessor;

impl ToolSchemaProcessor {
    pub fn extract_searchable_content(&self, tool: &ToolSchema) -> String;
    pub fn create_mcp_context(&self, tool: &ToolSchema, server: &McpServerInfo) -> McpToolContext;
    pub fn generate_embeddings_text(&self, context: &McpToolContext) -> String;
}
```

### 3. Enhanced Knowledge Store Interface

```rust
impl KnowledgeStore {
    // Existing methods remain unchanged
    
    // New MCP-specific methods
    pub async fn refresh_mcp_tools(&mut self) -> Result<String, String>;
    pub async fn get_mcp_servers(&self) -> Result<Vec<McpServerInfo>, KnowledgeError>;
    pub async fn search_with_type_filter(&self, query: &str, result_type: Option<SearchResultType>) -> Result<Vec<SearchResult>, KnowledgeError>;
}
```

## Indexing Strategy

### Content Generation for Embeddings

For each MCP tool, generate searchable content by combining:

```rust
fn generate_searchable_content(tool: &ToolSchema, server: &McpServerInfo) -> String {
    format!(
        "Tool: {} from {} server. Description: {}. Parameters: {}. Categories: {}",
        tool.name,
        server.name,
        tool.description.unwrap_or_default(),
        format_parameters(&tool.parameters),
        tool.categories.join(", ")
    )
}

fn format_parameters(params: &[ToolParameter]) -> String {
    params.iter()
        .map(|p| format!("{} ({}{})", 
            p.name, 
            p.param_type,
            if p.required { ", required" } else { "" }
        ))
        .collect::<Vec<_>>()
        .join(", ")
}
```

### Index Structure

```
Knowledge Index
├── Document Contexts (existing)
│   ├── Context 1: "My Project"
│   └── Context 2: "Documentation"
└── MCP Tool Contexts (new)
    ├── MCP Context 1: "weather-server/get_current_weather"
    ├── MCP Context 2: "file-server/read_file"
    └── MCP Context 3: "database-server/query_table"
```

## Integration with LLM Workflow

### Tool Selection Flow

1. **User Query**: "I need to check the weather in San Francisco"
2. **Knowledge Search**: Semantic search across all contexts (documents + MCP tools)
3. **Result Ranking**: MCP tools ranked by semantic similarity
4. **LLM Integration**: Top MCP tools provided to LLM as available functions
5. **Tool Execution**: LLM selects and calls appropriate MCP tool

### Function Calling Integration

```rust
// Extend existing tool calling infrastructure
impl McpToolContext {
    pub fn to_function_definition(&self) -> FunctionDefinition {
        FunctionDefinition {
            name: format!("{}_{}", self.server_name, self.tool_name),
            description: self.description.clone(),
            parameters: self.parameters_to_json_schema(),
        }
    }
}
```

## Error Handling

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("MCP server connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Tool schema parsing failed: {0}")]
    SchemaParseFailed(String),
    
    #[error("Configuration file error: {0}")]
    ConfigError(String),
    
    #[error("Server health check failed: {0}")]
    HealthCheckFailed(String),
}
```

### Fallback Strategy

- **Server Unavailable**: Skip indexing, log warning, continue with available servers
- **Schema Parse Error**: Skip problematic tool, continue with others
- **Search Failure**: Fall back to document-only search
- **Tool Execution Error**: Standard LLM tool calling error handling

## Testing Strategy

### Unit Tests
- MCP configuration parsing
- Tool schema processing
- Searchable content generation
- Context creation and management

### Integration Tests
- End-to-end MCP server discovery
- Tool indexing and search accuracy
- LLM integration with selected tools
- Error handling scenarios

### Performance Tests
- Index build time with 10-50 MCP servers
- Search response time
- Memory usage during indexing

## Security Considerations

### MCP Server Trust
- Only index servers explicitly configured in mcp.json
- Validate server responses before indexing
- Sanitize tool descriptions and parameters

### Tool Execution Safety
- Leverage existing LLM tool calling safety mechanisms
- No additional security layer needed (consistent with requirements)

## Implementation Phases

### Phase 1: Core Infrastructure
1. Create MCP discovery service
2. Implement tool schema processing
3. Extend KnowledgeStore with MCP support
4. Add MCP context types to semantic search client

### Phase 2: Integration
1. Integrate with existing knowledge search
2. Implement automatic MCP tool indexing
3. Add LLM function calling integration
4. Implement error handling and logging

### Phase 3: Polish & Testing
1. Add comprehensive test coverage
2. Performance optimization
3. Documentation and examples
4. User experience refinements

## Configuration

### mcp.json Structure (Existing)
```json
{
  "servers": {
    "weather-server": {
      "command": "weather-mcp-server",
      "args": ["--api-key", "..."],
      "env": {"API_KEY": "..."},
      "timeout": 30000,
      "disabled": false
    }
  }
}
```

### Knowledge Settings (New)
```rust
// Add to existing settings
pub enum Setting {
    // ... existing settings
    EnableMcpToolIndexing,
    McpIndexRefreshInterval,
    McpToolSearchWeight,
}
```

## Performance Characteristics

### Expected Performance (10-50 MCP servers)
- **Index Build Time**: 30-60 seconds (one-time)
- **Search Response**: <100ms
- **Memory Usage**: ~50-100MB additional
- **Storage**: ~10-50MB for tool index

### Optimization Strategies
- **Lazy Loading**: Index MCP tools on first search
- **Caching**: Cache tool schemas between sessions
- **Incremental Updates**: Only re-index changed servers
- **Background Refresh**: Periodic health checks and updates

## Monitoring and Observability

### Metrics to Track
- Number of MCP servers indexed
- Tool indexing success/failure rates
- Search query performance
- Tool selection accuracy
- MCP server health status

### Logging Strategy
- Info: MCP server discovery and indexing progress
- Warn: Server unavailable or schema parse errors
- Error: Critical failures that prevent tool indexing
- Debug: Detailed tool processing information

This design provides a comprehensive foundation for implementing the RAG tool for MCP servers while leveraging the existing infrastructure and maintaining consistency with current system patterns.
