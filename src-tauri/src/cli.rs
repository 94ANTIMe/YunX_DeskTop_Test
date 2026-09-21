//! 独立 CLI 与本地 function-call 工具入口。

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use futures_util::StreamExt;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, AppResult};
use crate::logger;
use crate::models::{ResolveSessionInfo, ShareFile, ShareFilePage};
use crate::state::AppState;

#[derive(Debug, Clone, Default)]
struct ParsedArgs {
    command: String,
    positionals: Vec<String>,
    options: BTreeMap<String, String>,
    flags: HashSet<String>,
    json: bool,
}

impl ParsedArgs {
    fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).map(String::as_str)
    }
    fn has_flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }
}

#[derive(Debug)]
struct CommandOutput {
    value: Value,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SafeLogRow {
    id: i64,
    time: i64,
    level: String,
    platform: String,
    action: String,
    message: String,
    detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadResult {
    path: String,
    filename: String,
    size: i64,
}

#[derive(Debug, Serialize)]
struct ToolError {
    code: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct ToolResponse {
    ok: bool,
    result: Option<Value>,
    error: Option<ToolError>,
}

/// CLI 进程入口；返回值可直接作为进程退出码。
pub async fn run() -> i32 {
    let raw_args = std::env::args().collect::<Vec<_>>();
    let parsed = match parse_args(&raw_args) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("错误：{message}");
            return 2;
        }
    };
    if parsed.command.is_empty() || parsed.command == "help" || parsed.has_flag("help") {
        print_help();
        return 0;
    }
    if parsed.command == "tools" {
        println!(
            "{}",
            serde_json::to_string_pretty(&tool_definitions()).unwrap_or_default()
        );
        return 0;
    }
    let state = match AppState::new(&default_data_dir()) {
        Ok(state) => state,
        Err(error) => return print_error(&error, parsed.json),
    };
    init_cli_sidecars();
    if parsed.command == "tool" {
        return run_tool(&state, &parsed).await;
    }
    match dispatch_command(&state, &parsed).await {
        Ok(output) => {
            if parsed.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output.value).unwrap_or_default()
                );
            } else {
                println!("{}", output.text);
            }
            0
        }
        Err(error) => print_error(&error, parsed.json),
    }
}

fn print_error(error: &AppError, json_output: bool) -> i32 {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({ "ok": false, "error": { "code": "cli", "message": error.to_string() } })
            )
            .unwrap_or_default()
        );
    } else {
        eprintln!("错误：{error}");
    }
    1
}

fn print_help() {
    println!("云析 CLI\n\n用法：\n  yunx resolve <链接> [--pwd <提取码>] [--json]\n  yunx files <链接> [--pwd <提取码>] [--dir-id <目录>] [--page <页码>] [--json]\n  yunx download <链接> --file-id <文件ID> [--pwd <提取码>] [--output <路径>] [--json]\n  yunx logs [--level <info|success|error>] [--limit <数量>] [--json]\n  yunx tools\n  yunx tool <resolve_share|list_files|download_file|get_logs> --input '<JSON>'\n\n说明：独立 CLI 每次命令内建立解析会话，不会把平台令牌写入磁盘。");
}

fn parse_args(args: &[String]) -> Result<ParsedArgs, String> {
    let command = args.get(1).cloned().unwrap_or_default();
    let mut parsed = ParsedArgs {
        command,
        ..Default::default()
    };
    let mut index = 2;
    while index < args.len() {
        let arg = &args[index];
        if let Some(raw) = arg.strip_prefix("--") {
            if raw == "json" || raw == "help" {
                parsed.flags.insert(raw.to_string());
                parsed.json |= raw == "json";
                index += 1;
                continue;
            }
            if let Some((key, value)) = raw.split_once('=') {
                parsed.options.insert(key.to_string(), value.to_string());
                index += 1;
                continue;
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("选项 --{raw} 缺少值"))?;
            if value.starts_with("--") {
                return Err(format!("选项 --{raw} 缺少值"));
            }
            parsed.options.insert(raw.to_string(), value.clone());
            index += 2;
        } else {
            parsed.positionals.push(arg.clone());
            index += 1;
        }
    }
    Ok(parsed)
}

async fn dispatch_command(state: &AppState, args: &ParsedArgs) -> AppResult<CommandOutput> {
    match args.command.as_str() {
        "resolve" => {
            let url = required_positional(args, 0, "分享链接")?;
            let info = resolve_share(state, url, args.option("pwd")).await?;
            Ok(CommandOutput {
                value: serde_json::to_value(&info)?,
                text: format_resolve(&info),
            })
        }
        "files" => {
            let url = required_positional(args, 0, "分享链接")?;
            let info = resolve_share(state, url, args.option("pwd")).await?;
            let page = match args.option("dir-id") {
                Some(dir_id) => {
                    crate::resolve::list_share_files(
                        state,
                        &info.session_key,
                        dir_id,
                        parse_i64(args.option("page"), "page")?.unwrap_or(1),
                    )
                    .await?
                }
                None => ShareFilePage {
                    files: info.files.clone(),
                    has_more: info.has_more,
                },
            };
            Ok(CommandOutput {
                value: serde_json::to_value(&page)?,
                text: format_files(&info.title, &page),
            })
        }
        "download" => {
            let url = required_positional(args, 0, "分享链接")?;
            let file_id = args
                .option("file-id")
                .ok_or_else(|| AppError::Api("缺少 --file-id".into()))?;
            let info = resolve_share(state, url, args.option("pwd")).await?;
            let file = info
                .files
                .iter()
                .find(|file| file.fid == file_id)
                .cloned()
                .ok_or_else(|| {
                    AppError::NotFound(
                        "文件不在首页列表中，请先用 files 查看目录或使用目录 ID".into(),
                    )
                })?;
            let link = crate::resolve::get_download_link(state, &info.session_key, &file).await?;
            let result = download_http(state, &link, args.option("output")).await?;
            Ok(CommandOutput {
                value: serde_json::to_value(&result)?,
                text: format!(
                    "下载完成：{}\n路径：{}\n大小：{} 字节",
                    result.filename, result.path, result.size
                ),
            })
        }
        "logs" => {
            let logs = list_safe_logs(
                state,
                args.option("level"),
                parse_i64(args.option("limit"), "limit")?.unwrap_or(100),
            )?;
            Ok(CommandOutput {
                value: serde_json::to_value(&logs)?,
                text: logs.iter().map(format_log).collect::<Vec<_>>().join("\n"),
            })
        }
        other => Err(AppError::Api(format!(
            "未知命令：{other}，使用 yunx help 查看帮助"
        ))),
    }
}

async fn run_tool(state: &AppState, args: &ParsedArgs) -> i32 {
    let name = match args.positionals.first() {
        Some(name) => name,
        None => return print_tool_error("invalid_request", "缺少工具名称".into()),
    };
    let input = match args.option("input") {
        Some(input) => input.to_string(),
        None => {
            let mut input = String::new();
            if std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).is_err() {
                input = "{}".into();
            }
            input
        }
    };
    let arguments = match serde_json::from_str::<Value>(&input) {
        Ok(value) => value,
        Err(error) => {
            return print_tool_error("invalid_json", format!("工具输入不是有效 JSON：{error}"))
        }
    };
    if let Err(message) = validate_tool_request(name, &arguments) {
        return print_tool_error("invalid_request", message);
    }
    match dispatch_tool(state, name, &arguments).await {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&ToolResponse {
                    ok: true,
                    result: Some(result),
                    error: None
                })
                .unwrap_or_default()
            );
            0
        }
        Err(error) => print_tool_error("tool_error", error.to_string()),
    }
}

fn print_tool_error(code: &'static str, message: String) -> i32 {
    println!(
        "{}",
        serde_json::to_string_pretty(&ToolResponse {
            ok: false,
            result: None,
            error: Some(ToolError { code, message })
        })
        .unwrap_or_default()
    );
    1
}

async fn dispatch_tool(state: &AppState, name: &str, args: &Value) -> AppResult<Value> {
    let object = args
        .as_object()
        .ok_or_else(|| AppError::Api("工具参数必须是 JSON 对象".into()))?;
    let url = object
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let password = object.get("password").and_then(Value::as_str);
    match name {
        "resolve_share" => Ok(serde_json::to_value(
            resolve_share(state, url, password).await?,
        )?),
        "list_files" => {
            let info = resolve_share(state, url, password).await?;
            if let Some(dir_id) = object.get("dirId").and_then(Value::as_str) {
                let page = object.get("page").and_then(Value::as_i64).unwrap_or(1);
                Ok(serde_json::to_value(
                    crate::resolve::list_share_files(state, &info.session_key, dir_id, page)
                        .await?,
                )?)
            } else {
                Ok(serde_json::to_value(ShareFilePage {
                    files: info.files,
                    has_more: info.has_more,
                })?)
            }
        }
        "download_file" => {
            let file_id = object
                .get("fileId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let info = resolve_share(state, url, password).await?;
            let file = info
                .files
                .iter()
                .find(|file| file.fid == file_id)
                .cloned()
                .ok_or_else(|| AppError::NotFound("文件不在首页列表中".into()))?;
            let link = crate::resolve::get_download_link(state, &info.session_key, &file).await?;
            Ok(serde_json::to_value(
                download_http(state, &link, object.get("output").and_then(Value::as_str)).await?,
            )?)
        }
        "get_logs" => {
            let level = object.get("level").and_then(Value::as_str);
            let limit = object.get("limit").and_then(Value::as_i64).unwrap_or(100);
            Ok(serde_json::to_value(list_safe_logs(state, level, limit)?)?)
        }
        _ => Err(AppError::Api("未知工具".into())),
    }
}

fn validate_tool_request(name: &str, args: &Value) -> Result<(), String> {
    if !["resolve_share", "list_files", "download_file", "get_logs"].contains(&name) {
        return Err(format!("未知工具：{name}"));
    }
    let object = args
        .as_object()
        .ok_or_else(|| "工具参数必须是 JSON 对象".to_string())?;
    if matches!(name, "resolve_share" | "list_files" | "download_file")
        && object
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| !url.trim().is_empty())
            .is_none()
    {
        return Err("缺少 url".into());
    }
    if name == "download_file"
        && object
            .get("fileId")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .is_none()
    {
        return Err("缺少 fileId".into());
    }
    Ok(())
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({ "type": "function", "function": { "name": "resolve_share", "description": "解析网盘分享链接并返回首页文件列表", "parameters": { "type": "object", "properties": { "url": { "type": "string" }, "password": { "type": "string" } }, "required": ["url"], "additionalProperties": false } } }),
        json!({ "type": "function", "function": { "name": "list_files", "description": "解析分享链接并列出根目录或指定目录文件", "parameters": { "type": "object", "properties": { "url": { "type": "string" }, "password": { "type": "string" }, "dirId": { "type": "string" }, "page": { "type": "integer", "minimum": 1 } }, "required": ["url"], "additionalProperties": false } } }),
        json!({ "type": "function", "function": { "name": "download_file", "description": "解析分享链接并下载首页列表中的指定文件", "parameters": { "type": "object", "properties": { "url": { "type": "string" }, "password": { "type": "string" }, "fileId": { "type": "string" }, "output": { "type": "string" } }, "required": ["url", "fileId"], "additionalProperties": false } } }),
        json!({ "type": "function", "function": { "name": "get_logs", "description": "读取最近的脱敏运行日志，用于判断解析和下载状态", "parameters": { "type": "object", "properties": { "level": { "type": "string", "enum": ["info", "success", "error"] }, "limit": { "type": "integer", "minimum": 1, "maximum": 500 } }, "additionalProperties": false } } }),
    ]
}

async fn resolve_share(
    state: &AppState,
    url: &str,
    password: Option<&str>,
) -> AppResult<ResolveSessionInfo> {
    if url.trim().is_empty() {
        return Err(AppError::Api("分享链接不能为空".into()));
    }
    crate::resolve::resolve_share(state, url, password).await
}

fn init_cli_sidecars() {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("baidupcs-x86_64-pc-windows-msvc.exe"));
            candidates.push(parent.join("baidupcs"));
        }
    }
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        candidates
            .push(PathBuf::from(manifest).join("binaries/baidupcs-x86_64-pc-windows-msvc.exe"));
    }
    if let Some(path) = candidates.into_iter().find(|path| path.exists()) {
        crate::baidupcs::init_sidecar(path);
    }
}

async fn download_http(
    state: &AppState,
    link: &crate::models::DownloadLink,
    output: Option<&str>,
) -> AppResult<DownloadResult> {
    let filename = crate::aria2::sanitize_out_path(&link.filename);
    let requested = output.map(PathBuf::from);
    let path = match requested {
        Some(path) if path.is_dir() => path.join(&filename),
        Some(path) => path,
        None => std::env::current_dir()?.join(&filename),
    };
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut request = state.http.get(&link.url);
    for (key, value) in &link.headers {
        let name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
            .map_err(|_| AppError::Api(format!("无效请求头：{key}")))?;
        let value = reqwest::header::HeaderValue::from_str(value)
            .map_err(|_| AppError::Api(format!("无效请求头值：{key}")))?;
        request = request.header(name, value);
    }
    state.log(
        logger::INFO,
        &link.platform,
        "cli-download",
        "开始下载",
        &link.filename,
    );
    let response = request.send().await?.error_for_status()?;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&path).await?;
    let mut downloaded = 0i64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        downloaded += chunk.len() as i64;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    if !link.cleanup_id.is_empty() {
        crate::resolve::cleanup_quark(state, &link.cleanup_id).await;
    }
    state.log(
        logger::SUCCESS,
        &link.platform,
        "cli-download",
        "下载完成",
        &format!("{} bytes", downloaded),
    );
    Ok(DownloadResult {
        path: path.display().to_string(),
        filename,
        size: downloaded,
    })
}

fn list_safe_logs(state: &AppState, level: Option<&str>, limit: i64) -> AppResult<Vec<SafeLogRow>> {
    let conn = state.db.lock().map_err(|_| AppError::Lock)?;
    Ok(logger::list(&conn, level, limit.clamp(1, 500))?
        .into_iter()
        .map(|row| SafeLogRow {
            id: row.id,
            time: row.time,
            level: row.level,
            platform: row.platform,
            action: row.action,
            message: redact_log_text(&row.message),
            detail: redact_log_text(&row.detail),
        })
        .collect())
}

fn format_resolve(info: &ResolveSessionInfo) -> String {
    format!(
        "平台：{}\n标题：{}\n会话：{}\n文件数：{}\n{}",
        info.platform,
        if info.title.is_empty() {
            "（无标题）"
        } else {
            &info.title
        },
        info.session_key,
        info.files.len(),
        format_file_lines(&info.files)
    )
}
fn format_files(title: &str, page: &ShareFilePage) -> String {
    format!(
        "标题：{}\n文件数：{}\n{}",
        if title.is_empty() {
            "（无标题）"
        } else {
            title
        },
        page.files.len(),
        format_file_lines(&page.files)
    )
}
fn format_file_lines(files: &[ShareFile]) -> String {
    files
        .iter()
        .map(|file| {
            format!(
                "{}\t{}\t{}",
                file.fid,
                if file.isdir { "目录" } else { "文件" },
                file.fname
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn format_log(log: &SafeLogRow) -> String {
    format!(
        "{} [{}] {} {} - {}",
        log.time, log.level, log.platform, log.action, log.message
    )
}
fn required_positional<'a>(args: &'a ParsedArgs, index: usize, label: &str) -> AppResult<&'a str> {
    args.positionals
        .get(index)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Api(format!("缺少{label}")))
}
fn parse_i64(value: Option<&str>, name: &str) -> AppResult<Option<i64>> {
    value
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| AppError::Api(format!("--{name} 必须是整数")))
        })
        .transpose()
}
fn default_data_dir() -> PathBuf {
    if let Some(app_data) = std::env::var_os("APPDATA") {
        return PathBuf::from(app_data).join("com.yunx.desktop");
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("com.yunx.desktop");
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("share")
        .join("com.yunx.desktop")
}

fn redact_log_text(text: &str) -> String {
    static CREDENTIALS: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let pattern = CREDENTIALS.get_or_init(|| Regex::new(r"(?i)(cookie|authorization|token|password|secret|access_token|refresh_token)\s*[:=]\s*[^;,&]+").expect("credential redaction regex"));
    pattern.replace_all(text, "$1=[REDACTED]").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_json_flag_and_resolve_arguments() {
        let args = vec![
            "yunx".into(),
            "resolve".into(),
            "https://pan.example/s/abc".into(),
            "--pwd".into(),
            "1234".into(),
            "--json".into(),
        ];
        let parsed = parse_args(&args).expect("resolve args should parse");
        assert_eq!(parsed.command, "resolve");
        assert_eq!(parsed.positionals, vec!["https://pan.example/s/abc"]);
        assert_eq!(parsed.option("pwd"), Some("1234"));
        assert!(parsed.json);
    }
    #[test]
    fn rejects_unknown_tool_name() {
        let error = validate_tool_request("delete_everything", &json!({})).unwrap_err();
        assert!(error.contains("未知工具"));
    }
    #[test]
    fn redacts_credentials_and_urls_from_log_text() {
        let text =
            "Cookie=abc; authorization: Bearer secret; url=https://example.test/file?token=abc&x=1";
        let redacted = redact_log_text(text);
        assert!(!redacted.contains("abc"));
        assert!(!redacted.contains("secret"));
        assert!(redacted.contains("[REDACTED]"));
        assert!(redacted.contains("x=1"));
    }
    #[test]
    fn exposes_stable_function_tool_names() {
        let definitions = tool_definitions();
        let names = definitions
            .iter()
            .filter_map(|tool| tool.get("function")?.get("name")?.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["resolve_share", "list_files", "download_file", "get_logs"]
        );
    }
}
