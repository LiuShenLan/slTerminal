//! 归属指纹:内容全等 + `.slterm-managed` marker(SHA-256)双通道判归属
//! (上游第三通道 legacy hash 白名单随双名兼容层整支裁剪——slTerminal 无
//! 历史安装)。安装/检视/卸载共用同一判断;被用户编辑过的同名文件报
//! Conflict 保留不动,设置页据此不把同名用户文件显示成已接入。

use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Install {
    Installed,
    Current,
    Conflict,
}

fn fingerprint(content: &[u8]) -> String {
    let mut fingerprint = String::with_capacity(64);
    for byte in Sha256::digest(content).iter() {
        let _ = write!(&mut fingerprint, "{byte:02x}");
    }
    fingerprint
}

fn marker_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.slterm-managed",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
    ))
}

fn read_if_present(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn is_owned(path: &Path, content: &[u8], expected: &[u8]) -> bool {
    let fingerprint = fingerprint(content);
    content == expected
        || std::fs::read_to_string(marker_path(path))
            .is_ok_and(|marker| marker.trim() == fingerprint)
}

/// 复用安装/卸载的归属判断;设置页不能把同名的用户文件显示成已接入。
pub(super) fn installed(path: &Path, content: &str) -> io::Result<bool> {
    match read_if_present(path)? {
        Some(bytes) => {
            if !is_owned(path, &bytes, content.as_bytes()) {
                return Err(io::Error::other(format!(
                    "Preserving edited or unmanaged integration: {}",
                    path.display()
                )));
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

/// 幂等安装:内容全等 + marker 在手 → Current;归属判不明 → Conflict
/// (不覆盖)。marker 只在内容原子写完后落下——崩溃不会把半个文件误认成
/// 我们所有,后续也绝不凭文件名覆盖用户同名文件。
pub(super) fn install(path: &Path, content: &str) -> io::Result<Install> {
    let current = read_if_present(path)?;
    if current
        .as_ref()
        .is_some_and(|bytes| !is_owned(path, bytes, content.as_bytes()))
    {
        return Ok(Install::Conflict);
    }
    let expected = fingerprint(content.as_bytes());
    let marker = marker_path(path);
    if current.as_deref() == Some(content.as_bytes())
        && std::fs::read_to_string(&marker).is_ok_and(|value| value.trim() == expected)
    {
        return Ok(Install::Current);
    }
    crate::atomic_file::write(path, content.as_bytes())?;
    crate::atomic_file::write(&marker, format!("{expected}\n").as_bytes())?;
    Ok(Install::Installed)
}

pub(super) fn remove(path: &Path, content: &str) -> io::Result<bool> {
    let Some(bytes) = read_if_present(path)? else {
        return Ok(false);
    };
    if !is_owned(path, &bytes, content.as_bytes()) {
        return Err(io::Error::other(format!(
            "Preserving edited or unmanaged integration: {}",
            path.display()
        )));
    }
    std::fs::remove_file(path)?;
    let marker = marker_path(path);
    if marker.is_file() {
        std::fs::remove_file(marker)?;
    }
    Ok(true)
}

#[cfg(test)]
mod managed_files_tests {
    use super::{Install, install, installed, remove};

    #[test]
    fn managed_file_install_is_idempotent_and_marks_ownership() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("slterm.js");

        assert_eq!(install(&path, "bridge v1").unwrap(), Install::Installed);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "bridge v1");
        assert!(installed(&path, "bridge v1").unwrap());
        assert_eq!(install(&path, "bridge v1").unwrap(), Install::Current);
        // marker 通道:内容被改写回全等之前,marker 也认归属。
        assert!(remove(&path, "bridge v1").unwrap());
        assert!(!path.exists());
        assert!(!directory.path().join("slterm.js.slterm-managed").exists());
    }

    #[test]
    fn edited_files_report_conflict_and_survive_update_and_remove() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("slterm.js");
        install(&path, "version one").unwrap();
        std::fs::write(&path, "version one with user changes").unwrap();

        assert_eq!(install(&path, "version two").unwrap(), Install::Conflict);
        assert!(remove(&path, "version two").is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "version one with user changes"
        );
    }

    #[test]
    fn unmanaged_same_name_files_are_neither_claimed_nor_shown_as_installed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("slterm.ts");
        std::fs::write(&path, "user extension").unwrap();

        // 设置页视角:同名用户文件报 Conflict(Err),不误显示为已接入。
        assert!(installed(&path, "our extension").is_err());
        assert_eq!(install(&path, "our extension").unwrap(), Install::Conflict);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "user extension");
        assert!(!path.with_extension("ts.slterm-managed").exists());
    }
}
