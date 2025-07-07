use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use tokio::runtime::Runtime;

use chat_cli::util::{
    mcp_processor::{McpDiscoveryService, ToolSchemaProcessor},
    test_utils::{create_mock_servers, create_temp_mcp_config, PerformanceTestUtils},
};

/// Benchmark MCP server discovery performance
fn bench_server_discovery(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("mcp_server_discovery");
    
    for server_count in [10, 50, 100].iter() {
        let servers = PerformanceTestUtils::create_large_server_set(*server_count);
        let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();
        
        group.bench_with_input(
            BenchmarkId::new("discover_servers", server_count),
            server_count,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let discovery_service = McpDiscoveryService::new().await.unwrap();
                    let result = discovery_service.discover_servers_from_path(&config_path, "bench").await.unwrap();
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark tool schema processing performance
fn bench_schema_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("mcp_schema_processing");
    
    let servers = PerformanceTestUtils::create_varied_servers();
    let processor = ToolSchemaProcessor::new();
    
    for server in servers.iter().take(3) { // Test with different server sizes
        let server_info = server.to_server_info();
        let tools_result = server.get_tools_result().unwrap();
        let tool_count = tools_result.tools.len();
        
        group.bench_with_input(
            BenchmarkId::new("extract_searchable_content", tool_count),
            &tool_count,
            |b, _| {
                b.iter(|| {
                    let content = processor.extract_searchable_content(&server_info, &tools_result);
                    black_box(content)
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("create_mcp_contexts", tool_count),
            &tool_count,
            |b, _| {
                b.iter(|| {
                    let contexts = processor.create_mcp_contexts(&server_info, &tools_result).unwrap();
                    black_box(contexts)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent MCP operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("mcp_concurrent_operations");
    group.measurement_time(Duration::from_secs(10));
    
    let servers = create_mock_servers();
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();
    
    for concurrency in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_discovery", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    
                    for i in 0..concurrency {
                        let path = config_path.clone();
                        let handle = tokio::spawn(async move {
                            let service = McpDiscoveryService::new().await.unwrap();
                            let scope = format!("bench-{}", i);
                            service.discover_servers_from_path(&path, &scope).await.unwrap()
                        });
                        handles.push(handle);
                    }
                    
                    let results = futures::future::join_all(handles).await;
                    black_box(results)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark memory allocation patterns
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("mcp_memory_usage");
    
    let servers = PerformanceTestUtils::create_large_server_set(50);
    let processor = ToolSchemaProcessor::new();
    
    group.bench_function("large_context_creation", |b| {
        b.iter(|| {
            let mut all_contexts = Vec::new();
            
            for server in &servers {
                let server_info = server.to_server_info();
                let tools_result = server.get_tools_result().unwrap();
                let contexts = processor.create_mcp_contexts(&server_info, &tools_result).unwrap();
                all_contexts.extend(contexts);
            }
            
            black_box(all_contexts)
        });
    });
    
    group.finish();
}

/// Benchmark string processing and content generation
fn bench_content_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("mcp_content_generation");
    
    let servers = PerformanceTestUtils::create_varied_servers();
    let processor = ToolSchemaProcessor::new();
    
    // Test with different content sizes
    for server in servers.iter().take(3) {
        let server_info = server.to_server_info();
        let tools_result = server.get_tools_result().unwrap();
        let tool_count = tools_result.tools.len();
        
        group.bench_with_input(
            BenchmarkId::new("content_generation", tool_count),
            &tool_count,
            |b, _| {
                b.iter(|| {
                    let mut total_content = String::new();
                    
                    for tool in &tools_result.tools {
                        if let Some(content) = processor.tool_to_searchable_text(&server_info, tool) {
                            total_content.push_str(&content);
                            total_content.push('\n');
                        }
                    }
                    
                    black_box(total_content)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark error handling overhead
fn bench_error_handling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("mcp_error_handling");
    
    // Create servers with some failures
    let mut servers = create_mock_servers();
    servers.extend(
        (0..10).map(|i| {
            chat_cli::util::test_utils::MockMcpServer::new(&format!("failing-server-{}", i))
                .with_failure("Benchmark failure")
        })
    );
    
    let (_temp_dir, config_path) = create_temp_mcp_config(&servers).unwrap();
    
    group.bench_function("discovery_with_failures", |b| {
        b.to_async(&rt).iter(|| async {
            let discovery_service = McpDiscoveryService::new().await.unwrap();
            let discovered_servers = discovery_service.discover_servers_from_path(&config_path, "bench").await.unwrap();
            let result = discovery_service.get_tool_schemas(&discovered_servers).await.unwrap();
            black_box(result)
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_server_discovery,
    bench_schema_processing,
    bench_concurrent_operations,
    bench_memory_usage,
    bench_content_generation,
    bench_error_handling
);

criterion_main!(benches);
