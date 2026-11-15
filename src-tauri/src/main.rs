// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    // `aurora --mcp` serves the read-only MCP server over stdio for Claude
    // Code, Codex and other MCP clients. It branches before Tauri starts, so
    // it opens no window and does not trip the single-instance guard.
    if std::env::args_os()
        .skip(1)
        .any(|argument| argument == "--mcp")
    {
        std::process::exit(aurora_lib::run_mcp_server());
    }
    aurora_lib::run()
}
