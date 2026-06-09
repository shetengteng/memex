//! MCP server：stdio transport + JSON-RPC dispatch + memex 工具实现。
//!
//! 子模块按职责分：
//!   * `transport` —— stdio 读写循环、`run_stdio` 入口。
//!   * `dispatch`  —— JSON-RPC method 路由、`initialize` / `tools/list`
//!     等 protocol 层 handler。
//!   * `tools`     —— `tools/call` 下的具体工具实现（search / get_session
//!     / list_recent / stats / get_project_context / list_sessions_by_range）。

mod dispatch;
mod transport;
mod tools;

pub use transport::run_stdio;

#[cfg(test)]
pub use dispatch::handle_request_for_test;
