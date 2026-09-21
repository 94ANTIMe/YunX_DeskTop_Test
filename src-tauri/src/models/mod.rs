//! IPC 契约模型（ADR-0007 C6）：按域拆分文件，此处统一再导出，
//! 外部 `crate::models::X` 路径保持不变（与前端 ipc.ts 成对修改的约定不变）。

mod account;
mod download;
mod platform;
mod resolve;
mod search;
mod settings;

pub use account::*;
pub use download::*;
pub use platform::*;
pub use resolve::*;
pub use search::*;
pub use settings::*;
