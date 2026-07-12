/// 获取 Windows 真实 build 号（F3 动态检测）
///
/// 通过 RtlGetNtVersionNumbers 获取，提取低 28 位（& 0x0FFFFFFF）。
/// 非 Windows 平台返回 Unknown 错误——前端需 try/catch fallback 到 21376。
use crate::error::AppError;

#[cfg(windows)]
#[tauri::command]
pub fn get_windows_build_number() -> Result<u32, AppError> {
    let (_, _, build) = nt_version::get();
    Ok(build & 0x0FFFFFFF) // 清除高 4 位构建类型标志（0xF=零售版），低 28 位为实际 build 号
}

#[cfg(not(windows))]
#[tauri::command]
pub fn get_windows_build_number() -> Result<u32, AppError> {
    Err(AppError::Unknown(
        "get_windows_build_number 仅 Windows 平台支持".to_string(),
    ))
}
