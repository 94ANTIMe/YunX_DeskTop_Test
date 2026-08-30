use serde::Serialize;

/// 统一错误类型：序列化为 { code, message } 传给前端
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("网络请求失败: {0}")]
    Network(#[from] reqwest::Error),
    #[error("状态锁获取失败（数据库连接被污染）")]
    Lock,
    #[error("{0}")]
    Api(String),
    #[error("凭据安全错误: {0}")]
    Crypto(String),
    #[error("功能未实现: {0}")]
    Unsupported(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let code = match self {
            AppError::Db(_) => "db",
            AppError::Io(_) => "io",
            AppError::Network(_) => "network",
            AppError::Lock => "lock",
            AppError::Api(_) => "api",
            AppError::Crypto(_) => "crypto",
            AppError::Unsupported(_) => "unsupported",
        };
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", code)?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Api(format!("响应解析失败: {e}"))
    }
}

pub type AppResult<T> = Result<T, AppError>;
