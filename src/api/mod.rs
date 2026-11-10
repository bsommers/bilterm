//! Automation, Robotics, JSON-RPC 2.0 Control Plane, Native MCP Server & Declarative Playbook Engine (`v1.1.0`–`v1.3.0`).
//!
//! Exposes 100% of Bilterm's `WindowManager`, `CompositePane`, `VtParser`, and `:colon` command
//! capabilities over:
//! 1. **JSON-RPC 2.0 Control Plane** (`AutomationControlPlane`, `run_headless_tcp_server`, `bilterm ctl`)
//! 2. **Model Context Protocol (MCP) Server** (`bilterm --mcp` over `stdio` JSON-RPC 2.0)
//! 3. **Real-Time Event Subscription Bus & Declarative Robot Playbooks** (`AutomationEvent`, `Playbook`, `bilterm --playbook`)

mod agent_wait;
pub mod client_view;
pub mod http;
pub mod http_web;
pub mod instance_registry;
pub mod live;
pub mod json;
pub mod mcp;
pub mod metered;
pub mod mcp_resources;
pub mod output_ring;
pub mod playbook;
pub mod rpc;
pub mod rpc_dispatch;
pub mod rpc_agents;
pub mod rpc_agent_ctl;
pub mod selector;
pub mod rpc_clipboard;
pub mod rpc_daemon;
pub mod rpc_handlers;
pub mod rpc_keys;
pub mod rpc_link;
pub mod rpc_media;
pub mod rpc_window;
pub mod rpc_session_map;
pub mod rpc_traffic;
pub mod rpc_coords;
pub mod rpc_context;
pub mod rpc_voice;
pub mod rpc_web;
pub mod server;
pub mod static_serve;
pub mod view_model;
pub mod web_surface;
pub mod ws;
pub mod ws_codec;
#[cfg(test)]
mod tests;

pub use http::*;
pub use json::*;
pub use mcp::*;
pub use playbook::*;
pub use rpc::*;
pub use server::*;
