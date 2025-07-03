# Implementation Prompt Plan

## Checklist
- [ ] Prompt 1: Set up MCP data models and core types
- [ ] Prompt 2: Implement MCP discovery service for reading mcp.json
- [ ] Prompt 3: Create tool schema processor for content generation
- [ ] Prompt 4: Extend semantic search client with MCP context support
- [ ] Prompt 5: Enhance KnowledgeStore with MCP integration
- [ ] Prompt 6: Implement automatic MCP tool indexing
- [ ] Prompt 7: Add LLM function calling integration
- [ ] Prompt 8: Implement error handling and logging
- [ ] Prompt 9: Add comprehensive test coverage
- [ ] Prompt 10: Wire everything together and test end-to-end

## Checkpoint Instructions

### Notes Checkpointing
After completing each prompt, create a checkpoint file at planning/implementation/checkpoint-prompt-N-complete.md with the following format:

**Template:**
```
# RAG Tool for MCP Servers - Implementation Checkpoint

**Status:** Prompt N Complete - Ready for Prompt N+1
**Date:** [Current Date]
**Next Step:** [Brief description of next prompt]

## Completed Work

### ✅ Prompt N: [Prompt Title]
- [Key achievement 1]
- [Key achievement 2]
- [Key achievement 3]

[Include previous completed prompts if this is not the first checkpoint]

## Technical Implementation Details

[Describe key technical decisions, architecture choices, or implementation patterns used]

## Test Results
[Include test results, coverage information, or validation outcomes]

## Next Steps - Prompt N+1: [Next Prompt Title]

### Objectives
[List the main objectives for the next prompt]

### Implementation Focus
[Describe the key areas of focus for the next implementation step]

## Files Status

### Core Implementation Files
- [List key files and their status: ✅ complete, 🔄 in progress, ⏳ placeholder]

### Test Files
- [List test files and their status]

## How to Continue

Reference this checkpoint and continue with:

> "Continue RAG Tool for MCP Servers implementation with Prompt N+1: [Next Prompt Title]. [Brief guidance for continuation]"

---

**Ready for Prompt N+1**: [Summary of readiness state]
```

## Prompts

### Prompt 1: Set up MCP data models and core types
Create the foundational data models and types needed for MCP tool indexing. Implement the core data structures that will represent MCP tools, servers, and search results throughout the system.

1. Create `McpToolContext` struct in `crates/semantic_search_client/src/types.rs` with fields for server name, tool name, description, parameters, and indexed content
2. Add `ToolParameter` struct to represent tool parameter information including name, type, description, and required flag
3. Create `McpServerConfig` struct to hold MCP server configuration from mcp.json
4. Extend `SearchResult` enum to include `McpTool` variant and add `mcp_tool_info` field
5. Add `SearchResultType` enum to distinguish between Document and McpTool results
6. Create comprehensive unit tests for all new data structures
7. Ensure all structs implement necessary traits (Debug, Clone, Serialize, Deserialize)

Focus on creating clean, well-documented data models that will serve as the foundation for the entire MCP integration. Make sure the types integrate seamlessly with existing semantic search client types.

### Prompt 2: Implement MCP discovery service for reading mcp.json
Create a service that can discover and parse MCP server configurations from the user's mcp.json file, providing the foundation for automatic MCP server indexing.

1. Create new module `crates/chat-cli/src/util/mcp_processor.rs` with `McpDiscoveryService` struct
2. Implement `discover_servers()` method to read and parse mcp.json configuration file
3. Add logic to filter only enabled MCP servers (where disabled != true)
4. Create `McpServerInfo` struct to represent discovered server information
5. Implement error handling for missing files, invalid JSON, and malformed configurations
6. Add comprehensive unit tests with mock mcp.json files
7. Include integration test that reads a real mcp.json configuration

Ensure the service gracefully handles various mcp.json formats and provides clear error messages for configuration issues. The implementation should be robust and handle edge cases like missing files or invalid JSON.

### Prompt 3: Create tool schema processor for content generation
Implement the logic to connect to MCP servers, retrieve tool schemas, and transform them into searchable content suitable for semantic indexing.

1. Extend `McpDiscoveryService` with `get_tool_schemas()` method to connect to MCP servers
2. Create `ToolSchemaProcessor` struct with methods for processing tool schemas
3. Implement `extract_searchable_content()` to generate embeddings-friendly text from tool schemas
4. Add `create_mcp_context()` method to transform tool schemas into `McpToolContext` objects
5. Include tool name, description, parameters, and server context in searchable content
6. Add server health checking with `validate_server_health()` method
7. Create comprehensive unit tests with mock MCP server responses
8. Add integration tests that work with the existing test MCP server

Focus on creating rich, semantic content that will enable accurate tool selection. The searchable content should include tool functionality, parameter information, and contextual details while remaining concise.

### Prompt 4: Extend semantic search client with MCP context support
Enhance the existing semantic search client to support MCP tool contexts alongside document contexts, enabling unified search across both types of content.

1. Add `McpToolContext` support to `crates/semantic_search_client/src/types.rs`
2. Extend `SemanticSearchClient` to handle MCP context creation and indexing
3. Add `add_mcp_context()` method to create MCP tool contexts from processed schemas
4. Modify search methods to return both document and MCP tool results
5. Ensure MCP contexts are properly persisted and can be loaded on restart
6. Add context type filtering capabilities for targeted searches
7. Create unit tests for MCP context operations
8. Add integration tests that verify MCP and document contexts work together

Maintain backward compatibility with existing document contexts while seamlessly integrating MCP tool contexts. The search experience should be unified and transparent to users.

### Prompt 5: Enhance KnowledgeStore with MCP integration
Extend the existing KnowledgeStore to automatically discover and index MCP tools from the user's configuration, integrating MCP functionality into the knowledge management system.

1. Add MCP-specific methods to `crates/chat-cli/src/util/knowledge_store.rs`
2. Implement `refresh_mcp_tools()` method to discover and index MCP servers from mcp.json
3. Add `get_mcp_servers()` method to retrieve information about indexed MCP servers
4. Extend existing search methods to include MCP tool results automatically
5. Implement background indexing of MCP tools with progress tracking
6. Add MCP server health monitoring and re-indexing capabilities
7. Create comprehensive unit tests for all new KnowledgeStore methods
8. Add integration tests that verify end-to-end MCP indexing workflow

Ensure the MCP integration feels natural and doesn't disrupt existing knowledge management workflows. Users should seamlessly get both document and tool results without additional complexity.

### Prompt 6: Implement automatic MCP tool indexing
Create the automatic indexing system that discovers MCP servers from mcp.json and indexes their tools without user intervention, making MCP tools immediately available for search.

1. Add automatic MCP discovery trigger when KnowledgeStore is initialized
2. Implement background indexing process that doesn't block user operations
3. Add configuration settings for MCP indexing behavior (enable/disable, refresh intervals)
4. Create incremental indexing that only updates changed MCP servers
5. Implement proper error handling for server connection failures
6. Add logging and progress reporting for indexing operations
7. Create unit tests for automatic indexing logic
8. Add integration tests that verify tools become searchable after indexing

Focus on making the indexing process robust and user-friendly. Users should see their MCP tools become available automatically without manual intervention, with clear feedback about the indexing process.

### Prompt 7: Add LLM function calling integration
Integrate MCP tool search results with the existing LLM function calling system, enabling the LLM to automatically discover and use relevant MCP tools based on user queries.

1. Extend search results to provide function definitions for discovered MCP tools
2. Implement `to_function_definition()` method on `McpToolContext` for LLM integration
3. Add MCP tool filtering and ranking logic for LLM function selection
4. Integrate with existing tool calling infrastructure in the chat system
5. Ensure MCP tools appear as available functions to the LLM during conversations
6. Add proper parameter mapping between MCP schemas and LLM function calls
7. Create unit tests for function definition generation
8. Add integration tests that verify LLM can discover and call MCP tools

The integration should be seamless - when users ask questions that could be answered by MCP tools, the LLM should automatically have access to relevant tools without explicit user commands.

### Prompt 8: Implement error handling and logging
Add comprehensive error handling and logging throughout the MCP integration, ensuring robust operation and clear diagnostics when issues occur.

1. Implement consistent error handling patterns across all MCP components
2. Add detailed logging for MCP server discovery, indexing, and search operations
3. Create user-friendly error messages for common failure scenarios
4. Add retry logic for transient MCP server connection failures
5. Implement graceful degradation when MCP servers are unavailable
6. Add monitoring and health check capabilities for indexed MCP servers
7. Create comprehensive error handling unit tests
8. Add integration tests for various failure scenarios

Ensure error handling is consistent with existing system patterns and provides helpful information for troubleshooting without overwhelming users with technical details.

### Prompt 9: Add comprehensive test coverage
Create thorough test coverage for all MCP integration components, ensuring reliability and maintainability of the implementation.

1. Add unit tests for all MCP data models and utility functions
2. Create integration tests for end-to-end MCP tool discovery and indexing
3. Add performance tests for search operations with mixed document/MCP content
4. Create mock MCP servers for reliable testing
5. Add tests for error conditions and edge cases
6. Implement tests for LLM integration and function calling
7. Add tests for configuration parsing and validation
8. Create tests for concurrent operations and thread safety

Focus on creating maintainable tests that will catch regressions and ensure the MCP integration continues to work correctly as the system evolves.

### Prompt 10: Wire everything together and test end-to-end
Complete the integration by connecting all components and conducting thorough end-to-end testing to ensure the RAG tool for MCP servers works seamlessly within the existing system.

1. Ensure all components are properly integrated and initialized
2. Add MCP tool indexing to the application startup sequence
3. Verify search operations return both documents and MCP tools appropriately
4. Test LLM conversations that utilize discovered MCP tools
5. Validate configuration management and settings integration
6. Conduct performance testing with realistic MCP server configurations
7. Add user documentation and examples for the new functionality
8. Create end-to-end integration tests that simulate real user workflows

Focus on ensuring the entire system works cohesively and provides a smooth user experience. The MCP integration should feel like a natural extension of the existing knowledge system rather than a separate feature.
