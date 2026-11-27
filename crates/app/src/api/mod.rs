//! 给 AI 用的绘图工具。窗口里的 HTTP API 和 `sciplot --mcp` 都调用这里。

mod http;
mod mcp;
mod tools;

pub use http::{start, ApiServer};
pub use mcp::serve;
pub use tools::{dispatch, readonly, ToolOutput};
