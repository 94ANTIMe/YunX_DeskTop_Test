import io, re
p = r"src/aria2/mod.rs"
s = io.open(p, encoding="utf-8").read()

def cut(start_marker, end_marker, tag):
    """按唯一文本锚点切除 [start, end) 块并返回"""
    global s
    i = s.index(start_marker)
    j = s.index(end_marker, i)
    block = s[i:j]
    s = s[:i] + s[j:]
    return block

# ---------- policy 块 1：TaskStatus/map_status/parse_status/transfer_tuning/stall_guard/http_task_options ----------
p1 = cut(
    "/// aria2 任务快照（tellStatus 关键字段）",
    "// ---------- 启动（sidecar） ----------",
    "policy1",
)

# ---------- policy 块 2：limit_str … sanitize_out_path（到 mark_enqueue_failed 前） ----------
p2 = cut(
    "fn limit_str(limit: i64) -> String {",
    "async fn mark_enqueue_failed(app: &AppHandle, id: i64, message: &str) {",
    "policy2",
)
# p2 尾部是 mark_enqueue_failed 前的注释/空行，把结尾裁到最后一个 }
# 直接保留原样（p2 末尾的空行无妨），但要剔除末尾可能带的下个函数文档注释——检查：截到最后 "}\n\n" 处
m = None
# 找 p2 中最后一个 "}\n"
idx = p2.rfind("\n}\n")
p2 = p2[: idx + 3]

# ---------- rpc 块 1：APP_HANDLE … is_connection_error ----------
r1 = cut(
    "static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();",
    "/// 引擎自愈互斥：并发 rpc_call 同时触发重拉时只执行一次，其余排队后复检",
    "rpc1",
)

# ---------- rpc 块 2：rpc_call ----------
r2 = cut(
    "/// 发起 JSON-RPC 请求：Unauthorized / 连接级失败走互斥自愈重拉（引擎崩溃不再永久停摆），",
    "/// aria2 任务快照（tellStatus 关键字段）",
    "rpc2",
)

# ---------- RPC_PORT 常量移入 rpc ----------
port_line = "const RPC_PORT: u16 = 16800;\n"
assert port_line in s
s = s.replace(port_line, "", 1)

# ---------- 写 policy.rs ----------
policy = '''//! aria2 策略纯函数（ADR-0007 C5）：状态映射、并发调参、路径净化、代理参数。
//! 全部为无副作用可单测的叶函数；测试随迁本文件。

use serde_json::Value;

use crate::models::Settings;

''' + p1 + "\n" + p2

# 可见性：policy 内 fn → pub(crate)
for fn_name in ["fn map_status", "fn parse_status", "fn transfer_tuning", "fn stall_guard_options",
                "fn http_task_options", "fn limit_str", "fn build_proxy_arg", "fn proxy_configured",
                "fn proxy_log_summary", "fn is_windows_reserved_name", "fn sanitize_out_path",
                "struct TaskStatus"]:
    policy = policy.replace("\n" + fn_name, "\npub(crate) " + fn_name)
    policy = policy.replace("\n\n" + fn_name, "\n\npub(crate) " + fn_name, 0)

io.open("src/aria2/policy.rs", "w", encoding="utf-8", newline="").write(policy)
print("policy.rs written")

# ---------- 写 rpc.rs ----------
rpc = '''//! aria2 JSON-RPC 客户端（ADR-0007 C5）：密钥持久化、底层调用、自愈触发与瞬时重试。
//! APP_HANDLE 全局句柄、端口查杀也在此（均服务 RPC 生命周期）。

use std::sync::OnceLock;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

use crate::error::{AppError, AppResult};

pub(crate) ''' + port_line + '''
''' + r1 + "\n" + r2

# 可见性提升
for name in ["fn rpc_secret", "fn rpc_url", "fn kill_stale_aria2_on_port", "fn rpc_http",
             "async fn rpc_call_raw", "fn is_transient_rpc_error", "fn is_connection_error",
             "async fn rpc_call", "static APP_HANDLE"]:
    rpc = rpc.replace("\n" + name, "\npub(crate) " + name)
io.open("src/aria2/rpc.rs", "w", encoding="utf-8", newline="").write(rpc)
print("rpc.rs written")

# ---------- mod.rs：声明子模块 + glob 再导出（外部路径不变） ----------
head_anchor = "use crate::state::AppState;\n"
assert head_anchor in s
s = s.replace(head_anchor, head_anchor + '''
mod policy;
mod rpc;

pub(crate) use policy::*;
pub(crate) use rpc::*;
''', 1)
# mod.rs 原头部 oncelock 等仍被 engine/tasks 使用，保留原 use
io.open(p, "w", encoding="utf-8", newline="").write(s)
print("mod.rs slimmer")
