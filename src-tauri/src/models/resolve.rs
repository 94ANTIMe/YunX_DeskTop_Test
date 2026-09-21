use serde::{Deserialize, Serialize};
/// 分享链接解析结果（对齐 Android ShareLinkParser.ParsedShare）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedShare {
    pub platform: String,
    pub share_id: String,
    pub pwd: String,
}

/// 分享内文件条目（对齐 Android ShareFile）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareFile {
    pub fid: String,
    pub fname: String,
    pub fsize: i64,
    pub isdir: bool,
    pub pdir_fid: String,
    /// 平台专属令牌（夸克/UC share_fid_token；123 为 "S3KeyFlag|Etag|StorageNode"）
    pub fid_token: String,
    pub modify_time: String,
}

/// 解析会话建立结果（首页）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveSessionInfo {
    pub session_key: String,
    pub platform: String,
    pub title: String,
    pub files: Vec<ShareFile>,
    pub has_more: bool,
}

/// 文件列表页
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareFilePage {
    pub files: Vec<ShareFile>,
    pub has_more: bool,
}

/// 文件夹收集结果（文件 + 相对目录，用于还原目录结构保存）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectedFile {
    #[serde(flatten)]
    pub file: ShareFile,
    /// 相对目录路径（含子目录名，如 `影视/2024`；根级文件为空）
    pub rel_dir: String,
}
