use serde::Serialize;
/// PanSou 搜索结果条目（merged_by_type 分组内的一项）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchItem {
    /// 网盘类型：baidu/quark/aliyun/...
    pub r#type: String,
    /// 分享链接
    pub url: String,
    /// 提取码（可能为空）
    pub password: String,
    /// 标题/备注
    pub note: String,
    /// 来源（tg:频道 / plugin:插件）
    pub source: String,
}
