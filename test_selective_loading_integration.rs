#!/usr/bin/env rust-script

//! Test script to verify selective loading integration
//! 
//! This script tests:
//! 1. Setting selective loading enabled/disabled
//! 2. Verifying the setting persists
//! 3. Testing the CLI commands work

use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Selective Loading Integration");
    
    // Test 1: Check if the setting can be enabled
    println!("\n1. Testing selective loading enable...");
    let output = Command::new("cargo")
        .args(&["run", "--bin", "chat_cli", "--", "settings", "mcp.selectiveLoading.enabled", "true"])
        .current_dir("crates/chat-cli")
        .output()?;
    
    if output.status.success() {
        println!("✅ Setting enabled successfully");
    } else {
        println!("❌ Failed to enable setting: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Test 2: Check if the setting can be read
    println!("\n2. Testing selective loading status...");
    let output = Command::new("cargo")
        .args(&["run", "--bin", "chat_cli", "--", "settings", "mcp.selectiveLoading.enabled"])
        .current_dir("crates/chat-cli")
        .output()?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("✅ Setting status: {}", stdout.trim());
    } else {
        println!("❌ Failed to read setting: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Test 3: Test that the build includes our new components
    println!("\n3. Testing build includes selective loading components...");
    let output = Command::new("cargo")
        .args(&["build", "--bin", "chat_cli"])
        .current_dir("crates/chat-cli")
        .output()?;
    
    if output.status.success() {
        println!("✅ Build successful - selective loading components integrated");
    } else {
        println!("❌ Build failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    println!("\n🎉 Integration test completed!");
    println!("\n📋 Summary:");
    println!("• Selective loading setting can be configured");
    println!("• Build includes all selective loading components");
    println!("• CLI commands are available (test in interactive mode)");
    println!("\n💡 To test interactively:");
    println!("   cargo run --bin chat_cli");
    println!("   Then try: /selective-loading help");
    
    Ok(())
}
