
/// 网盘与通用下载平台标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Quark,
    Uc,
    Xunlei,
    Baidu,
    C139,
    Pan123,
    Direct,
    Magnet,
}

impl Platform {
    /// 小写标识（DB / IPC 传输用）
    pub fn key(&self) -> &'static str {
        match self {
            Platform::Quark => "quark",
            Platform::Uc => "uc",
            Platform::Xunlei => "xunlei",
            Platform::Baidu => "baidu",
            Platform::C139 => "c139",
            Platform::Pan123 => "pan123",
            Platform::Direct => "direct",
            Platform::Magnet => "magnet",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "quark" => Platform::Quark,
            "uc" => Platform::Uc,
            "xunlei" => Platform::Xunlei,
            "baidu" => Platform::Baidu,
            "c139" => Platform::C139,
            "pan123" => Platform::Pan123,
            "direct" => Platform::Direct,
            "magnet" => Platform::Magnet,
            _ => return None,
        })
    }
}
