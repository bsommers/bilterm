//! Automation, Robotics, JSON-RPC 2.0 Control Plane, Native MCP Server & Declarative Playbook Engine (`v1.1.0`–`v1.3.0`).
//!
//! Exposes 100% of Bilterm's `WindowManager`, `CompositePane`, `VtParser`, and `:colon` command
//! capabilities over:
//! 1. **JSON-RPC 2.0 Control Plane** (`AutomationControlPlane`, `run_headless_tcp_server`, `bilterm ctl`)
//! 2. **Model Context Protocol (MCP) Server** (`bilterm --mcp` over `stdio` JSON-RPC 2.0)
//! 3. **Real-Time Event Subscription Bus & Declarative Robot Playbooks** (`AutomationEvent`, `Playbook`, `bilterm --playbook`)

pub mod json;
pub mod mcp;
pub mod playbook;
pub mod rpc;
pub mod rpc_handlers;
pub mod server;
#[cfg(test)]
mod tests;

pub use json::*;
pub use mcp::*;
pub use playbook::*;
pub use rpc::*;
pub use server::*;
