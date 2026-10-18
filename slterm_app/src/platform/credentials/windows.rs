//! Win32 凭据管理器原语。
//!
//! `CredReadW`/`CredWriteW`/`CredDeleteW` + `CRED_TYPE_GENERIC` +
//! `CRED_PERSIST_LOCAL_MACHINE`。错误归一:读取命中 `ERROR_NOT_FOUND` →
//! `Ok(None)`,删除命中 → `Ok(())`(幂等);其余后端错误原样上抛。写入的
//! blob/target/user 缓冲区在系统调用返回后一律 `fill(0)`,不把明文留在
//! 栈/堆缓冲。本层只返回裸字节;`Zeroizing` 出域句柄由 `crate::secrets`
//! 门面完成。

use std::ffi::c_void;
use std::io;
use std::ptr::null_mut;

use windows::Win32::Foundation::{ERROR_NOT_FOUND, FILETIME};
use windows::Win32::Security::Credentials::{
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
    CredWriteW,
};
use windows::core::{HRESULT, PCWSTR, PWSTR};

/// 构造 UTF-16 NUL 结尾缓冲区。
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `ERROR_NOT_FOUND` 归一判据(windows crate 把 Win32 错误折成 HRESULT)。
fn is_not_found(error: &windows::core::Error) -> bool {
    error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0)
}

/// 后端错误消息只含操作名与 target,禁止拼接凭据值(SEC-18 日志红线)。
fn backend_error(operation: &str, target: &str, error: windows::core::Error) -> io::Error {
    io::Error::other(format!(
        "credential manager {operation} failed for {target}: {error}"
    ))
}

/// 读取凭据;不存在返回 `Ok(None)`。
pub(crate) fn load_secret(target_name: &str) -> io::Result<Option<Vec<u8>>> {
    let target = wide(target_name);
    let mut raw = null_mut();
    if let Err(error) =
        unsafe { CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut raw) }
    {
        if is_not_found(&error) {
            return Ok(None);
        }
        return Err(backend_error("read", target_name, error));
    }
    // raw 归 CredFree 释放;blob 复制出域后由上层包 Zeroizing。
    let result = unsafe {
        let credential = &*raw;
        if credential.CredentialBlob.is_null() || credential.CredentialBlobSize == 0 {
            None
        } else {
            Some(
                std::slice::from_raw_parts(
                    credential.CredentialBlob,
                    credential.CredentialBlobSize as usize,
                )
                .to_vec(),
            )
        }
    };
    unsafe { CredFree(raw as *const c_void) };
    Ok(result)
}

/// 写入凭据(覆盖同 target 旧值)。
pub(crate) fn save_secret(target_name: &str, username: &str, secret: &[u8]) -> io::Result<()> {
    if secret.len() > u32::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential too long",
        ));
    }
    let mut target = wide(target_name);
    let mut user = wide(username);
    let mut blob = secret.to_vec();
    let credential = CREDENTIALW {
        Flags: Default::default(),
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_mut_ptr()),
        Comment: PWSTR::null(),
        LastWritten: FILETIME::default(),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: null_mut(),
        TargetAlias: PWSTR::null(),
        UserName: PWSTR(user.as_mut_ptr()),
    };
    let result = unsafe { CredWriteW(&credential, 0) };
    // 无论成败都擦除本地明文缓冲;系统调用已完成,指针不再被使用。
    blob.fill(0);
    target.fill(0);
    user.fill(0);
    result.map_err(|error| backend_error("write", target_name, error))
}

/// 删除凭据;不存在视为成功(幂等)。
pub(crate) fn delete_secret(target_name: &str) -> io::Result<()> {
    let target = wide(target_name);
    match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(()),
        Err(error) if is_not_found(&error) => Ok(()),
        Err(error) => Err(backend_error("delete", target_name, error)),
    }
}

#[cfg(test)]
mod windows_tests {
    use super::*;
    use windows::Win32::Foundation::ERROR_ACCESS_DENIED;

    #[test]
    fn wide_encodes_utf16_with_nul_terminator() {
        assert_eq!(wide("Slterm"), vec![0x53, 0x6c, 0x74, 0x65, 0x72, 0x6d, 0]);
        assert_eq!(wide("凭"), vec![0x51ed, 0]);
    }

    #[test]
    fn not_found_matches_only_the_not_found_hresult() {
        let not_found = windows::core::Error::from_hresult(HRESULT::from_win32(ERROR_NOT_FOUND.0));
        assert!(is_not_found(&not_found));
        let denied = windows::core::Error::from_hresult(HRESULT::from_win32(ERROR_ACCESS_DENIED.0));
        assert!(!is_not_found(&denied));
    }
}
