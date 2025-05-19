// This file is used to share common utilities and configuration for tests

use std::env;

// Initialize the test environment
pub fn init() {
    // Set up logging for tests
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info,mcp_language_server_rust=debug");
    }
    
    // Ensure that the test runs in a deterministic environment
    if env::var("RUST_BACKTRACE").is_err() {
        env::set_var("RUST_BACKTRACE", "1");
    }
}