//! Windows DPAPI CurrentUser 封装：恢复点正文本机加密，不需要重输口令。
//!
//! 只做加密原语；恢复点结构、指纹与回滚规则归 `encrypted_backup::recovery`。

use std::io;

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

fn input_blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    }
}

fn take_output(output: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    // SAFETY: CryptProtectData/CryptUnprotectData 用 LocalAlloc 返回输出，
    // 复制后立即 LocalFree；空指针 + 0 长度视为空。
    unsafe {
        let bytes = if output.pbData.is_null() || output.cbData == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        if !output.pbData.is_null() {
            let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        }
        bytes
    }
}

/// DPAPI CurrentUser 加密：绑定当前用户账户，其他账户无法解密。
pub(crate) fn protect(plaintext: &[u8]) -> io::Result<Vec<u8>> {
    let input = input_blob(plaintext);
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: 输入/输出 blob 均由本函数持有；不弹 UI，避免恢复流程阻塞前台。
    unsafe {
        CryptProtectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(io::Error::other)?;
    }
    Ok(take_output(output))
}

/// DPAPI CurrentUser 解密：密文被篡改或跨账户时返回错误。
pub(crate) fn unprotect(ciphertext: &[u8]) -> io::Result<Vec<u8>> {
    let input = input_blob(ciphertext);
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: 同 protect；输出由 LocalFree 回收。
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(io::Error::other)?;
    }
    Ok(take_output(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实 DPAPI 往返：明文零散落、密文可解密回原文、篡改被拒绝。
    #[test]
    fn dpapi_current_user_round_trip_and_tamper_rejection() {
        let plaintext = b"recovery point body";
        let packet = protect(plaintext).unwrap();
        assert_ne!(packet, plaintext, "密文不得包含明文");
        assert_eq!(unprotect(&packet).unwrap(), plaintext);

        let mut tampered = packet.clone();
        let middle = tampered.len() / 2;
        tampered[middle] ^= 0xff;
        assert!(unprotect(&tampered).is_err(), "篡改密文必须拒绝解密");
        assert!(unprotect(b"not-a-dpapi-packet").is_err());
    }
}
