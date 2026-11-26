//! 维护期恢复点原语:恢复点只在本机 DPAPI 加密落盘,不经会过滤敏感字段的
//! 可分享收集器;运行期不直接替换任何目标文件。
//!
//! 恢复与撤回都只经维护请求在启动维护期执行；类别与目标名集合必须与登记的
//! 恢复事实一致,外部导入文件不能借撤回扩展白名单。本模块只提供恢复点读写、
//! 原现场采样与回写原语,由维护执行器独占调用。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::*;

/// 恢复目标的类别与根内相对名共同构成身份，禁止跨根伪造。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RestoreTarget {
    pub(super) category: BackupCategory,
    pub(super) name: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct OriginalFile {
    pub(super) target: RestoreTarget,
    /// None = 恢复前该文件不存在;撤回时删除,而不是留下恢复新建的文件。
    pub(super) bytes: Option<Vec<u8>>,
    /// 恢复成功后的文件指纹；撤回时逐项比较，后续修改项保留当前内容。
    #[serde(default)]
    pub(super) after: Option<[u8; 32]>,
}

/// 恢复点正文加密（DPAPI CurrentUser）：明文不落盘。
pub(super) fn encode_point(originals: &[OriginalFile]) -> Result<Vec<u8>, String> {
    let plaintext = Zeroizing::new(
        serde_json::to_vec(originals).map_err(|error| format!("序列化恢复点失败: {error}"))?,
    );
    crate::platform::dpapi::protect(&plaintext)
        .map_err(|error| format!("恢复点 DPAPI 加密失败: {error}"))
}

/// 恢复点正文明文；篡改密文或跨账户解密失败。
pub(super) fn decode_point(packet: &[u8]) -> Result<Vec<OriginalFile>, String> {
    let plaintext = Zeroizing::new(
        crate::platform::dpapi::unprotect(packet)
            .map_err(|error| format!("恢复点 DPAPI 解密失败: {error}"))?,
    );
    serde_json::from_slice(&plaintext).map_err(|error| format!("恢复点内容无效: {error}"))
}

/// 维护期恢复点落盘：文件名由请求拥有，目录固定在本地根恢复点目录。
pub(super) fn write_point(
    paths: &AppPaths,
    file_name: &str,
    originals: &[OriginalFile],
) -> Result<PathBuf, String> {
    let name = Path::new(file_name);
    if file_name.is_empty()
        || name.is_absolute()
        || name
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("恢复点文件名无效".to_owned());
    }
    let directory = restore_path(paths.local_root(), "backup-recovery")?;
    fs::create_dir_all(&directory).map_err(|error| format!("创建恢复点目录失败: {error}"))?;
    let path = directory.join(name);
    let packet = encode_point(originals)?;
    crate::atomic_file::write(&path, &packet)
        .map_err(|error| format!("保存恢复点失败: {error}"))?;
    Ok(path)
}

pub(super) fn read_original(paths: &AppPaths, entry: &BackupEntry) -> Result<OriginalFile, String> {
    let name = &entry.name;
    let path = restore_path(category_root(paths, entry.category), name)?;
    if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(format!("备份目标是符号链接: {name}"));
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(format!("读取恢复前现场 {name} 失败: {error}")),
    };
    Ok(OriginalFile {
        target: RestoreTarget {
            category: entry.category,
            name: name.clone(),
        },
        bytes,
        after: None,
    })
}

pub(super) fn write_originals(paths: &AppPaths, originals: &[OriginalFile]) -> Result<(), String> {
    let restore_paths = originals
        .iter()
        .map(|file| {
            restore_path(
                category_root(paths, file.target.category),
                &file.target.name,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (original, path) in originals.iter().zip(restore_paths) {
        match &original.bytes {
            Some(bytes) => crate::atomic_file::write(&path, bytes)
                .map_err(|error| format!("撤回 {} 失败: {error}", original.target.name))?,
            None => match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!("撤回 {} 失败: {error}", original.target.name));
                }
            },
        }
    }
    Ok(())
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    const PASSPHRASE: &str = "correct horse";

    fn split_paths(root: &Path) -> AppPaths {
        AppPaths::new(root.join("config"), root.join("local")).unwrap()
    }

    fn selection() -> BackupSelection {
        BackupSelection::from_categories([BackupCategory::Settings])
    }

    fn archive() -> BackupArchive {
        BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: vec![BackupCategory::Settings],
            },
            entries: vec![
                BackupEntry {
                    category: BackupCategory::Settings,
                    name: "settings.json".into(),
                    bytes: br#"{"appearance":{"theme":"Nord"}}"#.to_vec(),
                },
                BackupEntry {
                    category: BackupCategory::Settings,
                    name: "themes/018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b.slterm-theme.json".into(),
                    bytes: crate::theme_library::builtin_documents()
                        .first()
                        .expect("内置主题文档非空")
                        .to_json_bytes()
                        .expect("内置主题文档可序列化"),
                },
            ],
        }
    }

    /// 端到端只覆盖收集 → 密封 → 打开；落盘由维护期执行器独占。
    #[test]
    fn integration_encrypted_backup_round_trips_without_touching_disk() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(
            paths.config_root().join("settings.json"),
            br#"{"appearance":{"theme":"Linear"}}"#,
        )
        .unwrap();
        fs::create_dir_all(paths.config_root().join("themes")).unwrap();
        fs::write(
            paths
                .config_root()
                .join("themes/018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b.slterm-theme.json"),
            crate::theme_library::builtin_documents()
                .first()
                .expect("内置主题文档非空")
                .to_json_bytes()
                .expect("内置主题文档可序列化"),
        )
        .unwrap();
        fs::create_dir_all(paths.config_root().join("themes")).unwrap();
        write_theme(&paths);

        let archive = collect_from(&paths, selection()).unwrap();
        let packet = seal(&archive, PASSPHRASE).unwrap();
        let opened = open(&packet, PASSPHRASE).unwrap();
        assert_eq!(opened, archive);
        assert_eq!(
            fs::read(paths.config_root().join("settings.json")).unwrap(),
            br#"{"appearance":{"theme":"Linear"}}"#,
            "打开包不得改动现场"
        );
    }

    /// 恢复点 DPAPI 加密：明文不落盘，读回与采样内容一致。
    #[test]
    fn recovery_point_is_encrypted_and_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(
            paths.settings_path(),
            br#"{"appearance":{"theme":"Linear-SECRET-MARKER"}}"#,
        )
        .unwrap();
        let entry = &archive().entries[0];
        let original = read_original(&paths, entry).unwrap();
        let path = write_point(&paths, "point.slterm-recovery", &[original]).unwrap();
        let packet = fs::read(&path).unwrap();
        // DPAPI 报文无固定魔数；加密性由「不含明文 + 篡改拒绝」两条断言锁定。
        assert!(
            !packet
                .windows("Linear-SECRET-MARKER".len())
                .any(|window| window == b"Linear-SECRET-MARKER"),
            "恢复点不得以明文携带本机原值"
        );
        let restored = decode_point(&packet).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].target.category, BackupCategory::Settings);
        assert_eq!(restored[0].target.name, "settings.json");
        assert_eq!(
            restored[0].bytes.as_deref(),
            Some(&br#"{"appearance":{"theme":"Linear-SECRET-MARKER"}}"#[..])
        );

        let mut tampered = packet;
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        assert!(decode_point(&tampered).is_err(), "篡改恢复点必须解密失败");
    }

    /// 恢复前不存在的目标以 None 记录，回写时删除恢复新建的文件。
    #[test]
    fn write_originals_restores_existing_and_removes_absent_targets() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(paths.settings_path(), b"current").unwrap();
        let originals = vec![
            OriginalFile {
                target: RestoreTarget {
                    category: BackupCategory::Settings,
                    name: "settings.json".into(),
                },
                bytes: Some(b"before".to_vec()),
                after: None,
            },
            OriginalFile {
                target: RestoreTarget {
                    category: BackupCategory::Settings,
                    name: "themes/018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b.slterm-theme.json".into(),
                },
                bytes: None,
                after: None,
            },
        ];
        fs::create_dir_all(paths.config_root().join("themes")).unwrap();
        fs::write(
            paths
                .config_root()
                .join("themes/018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b.slterm-theme.json"),
            b"created",
        )
        .unwrap();
        write_originals(&paths, &originals).unwrap();
        assert_eq!(fs::read(paths.settings_path()).unwrap(), b"before");
        assert!(
            !paths
                .config_root()
                .join("themes/018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b.slterm-theme.json")
                .exists()
        );
    }

    fn write_theme(paths: &AppPaths) {
        let theme = crate::theme_library::builtin_documents()
            .first()
            .expect("内置主题文档非空")
            .to_json_bytes()
            .expect("内置主题文档可序列化");
        fs::write(
            paths
                .config_root()
                .join("themes/018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b.slterm-theme.json"),
            theme,
        )
        .unwrap();
    }
}
