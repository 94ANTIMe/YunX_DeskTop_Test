//! 凭据加密：Windows DPAPI（本机用户级，离线不可跨机器/跨账户恢复）。
//! 加密文本带 `dpapi1:` 前缀；`decrypt` 对无前缀的旧明文返回 `None`，
//! 由调用方就地重写为密文（迁移）。非 Windows 平台透明透传，保持开发/CI 可用。

#[cfg(target_os = "windows")]
mod impls {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    use crate::error::AppError;

    /// 已加密数据的标识前缀
    pub const PREFIX: &str = "dpapi1:";

    /// DPAPI 加密（CRYPTPROTECT_UI_FORBIDDEN：禁止弹认证 UI）
    pub fn encrypt(plain: &str) -> crate::error::AppResult<String> {
        let bytes = plain.as_bytes();
        let in_blob =
            CRYPT_INTEGER_BLOB { pbData: bytes.as_ptr() as *mut u8, cbData: bytes.len() as u32 };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        let result = unsafe {
            CryptProtectData(
                &in_blob,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            )
        };
        if result.is_err() {
            return Err(AppError::Crypto(format!("DPAPI 加密失败: {result:?}")));
        }
        let blob = unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) };
        let b64 = crate::api::b64_encode(blob);
        unsafe {
            LocalFree(Some(HLOCAL(out_blob.pbData as *mut core::ffi::c_void)));
        }
        Ok(format!("{PREFIX}{b64}"))
    }

    /// DPAPI 解密。`Ok(None)` = 旧明文（无前缀），需迁移；
    /// `Err` = 带前缀但解密失败（系统重装 / 更换用户 / 数据损坏）。
    pub fn decrypt(encoded: &str) -> crate::error::AppResult<Option<String>> {
        if !encoded.starts_with(PREFIX) {
            return Ok(None);
        }
        let b64 = &encoded[PREFIX.len()..];
        let blob = crate::api::b64_decode(b64)
            .ok_or_else(|| AppError::Crypto("凭据编码损坏".into()))?;
        let in_blob = CRYPT_INTEGER_BLOB { pbData: blob.as_ptr() as *mut u8, cbData: blob.len() as u32 };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        let result = unsafe {
            CryptUnprotectData(&in_blob, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out_blob)
        };
        if result.is_err() {
            return Err(AppError::Crypto("凭据解密失败（系统重装或更换用户后无法恢复）".into()));
        }
        let out = unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) };
        let s = String::from_utf8_lossy(out).into_owned();
        unsafe {
            LocalFree(Some(HLOCAL(out_blob.pbData as *mut core::ffi::c_void)));
        }
        Ok(Some(s))
    }
}

#[cfg(not(target_os = "windows"))]
mod impls {
    // 非 Windows：无 DPAPI，透明透传保持功能可用（仅开发/CI 用）
    pub const PREFIX: &str = "dpapi1:";

    pub fn encrypt(plain: &str) -> crate::error::AppResult<String> {
        Ok(plain.to_string())
    }

    pub fn decrypt(encoded: &str) -> crate::error::AppResult<Option<String>> {
        if encoded.starts_with(PREFIX) {
            // 跨平台环境无法处理 Windows 密文，按旧明文语义返回 None
            Ok(None)
        } else {
            Ok(Some(encoded.to_string()))
        }
    }
}

pub use impls::{decrypt, encrypt};