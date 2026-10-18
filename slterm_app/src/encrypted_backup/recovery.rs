//! 选择性恢复与其加密恢复点:UI 只持句柄,不承担文件
//! 写入规则。
//!
//! 恢复点只在本机加密落盘,不经会过滤敏感字段的可分享收集器;撤回时目标
//! 名集合必须与该次已验证恢复完全一致(`RestorePoint::names` 私有只读),
//! 外部导入文件不能借撤回扩展白名单。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

/// 一次选择性恢复之前的本机现场句柄。
#[derive(Clone, Debug)]
pub(crate) struct RestorePoint {
    pub(crate) path: PathBuf,
    /// 目标名集合:私有,写入规则不暴露给 UI。
    names: Vec<String>,
}

impl RestorePoint {
    /// 恢复点文件路径(UI 展示/清理用)。
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// 该次恢复触及的目标名只读清单(UI 展示撤回范围用)。
    pub(crate) fn names(&self) -> &[String] {
        &self.names
    }
}

#[derive(Serialize, Deserialize)]
struct OriginalFile {
    name: String,
    /// None = 恢复前该文件不存在;撤回时删除,而不是留下恢复新建的文件。
    bytes: Option<Vec<u8>>,
}

fn read_original(root: &Path, name: &str) -> Result<OriginalFile, String> {
    let path = restore_path(root, name)?;
    if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(format!("备份目标是符号链接: {name}"));
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(format!("读取恢复前现场 {name} 失败: {error}")),
    };
    Ok(OriginalFile {
        name: name.to_owned(),
        bytes,
    })
}

fn write_originals(root: &Path, originals: &[OriginalFile]) -> Result<(), String> {
    let paths = originals
        .iter()
        .map(|file| restore_path(root, &file.name))
        .collect::<Result<Vec<_>, _>>()?;
    for (original, path) in originals.iter().zip(paths) {
        match &original.bytes {
            Some(bytes) => crate::atomic_file::write(&path, bytes)
                .map_err(|error| format!("撤回 {} 失败: {error}", original.name))?,
            None => match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(format!("撤回 {} 失败: {error}", original.name)),
            },
        }
    }
    Ok(())
}

/// 选择性恢复:先采原字节与「不存在」状态 → 本机加密落盘恢复点 →
/// 原子写恢复;恢复失败自动回滚。
pub(crate) fn restore_selected(
    archive: &BackupArchive,
    selection: BackupSelection,
    passphrase: &str,
) -> Result<RestorePoint, String> {
    restore_selected_at(
        crate::platform::dirs::data_dir(),
        archive,
        selection,
        passphrase,
    )
}

fn restore_selected_at(
    root: &Path,
    archive: &BackupArchive,
    selection: BackupSelection,
    passphrase: &str,
) -> Result<RestorePoint, String> {
    validate_archive(archive)?;
    let categories: HashSet<_> = selection.categories().collect();
    if categories.is_empty() {
        return Err("至少选择一个恢复类别".to_owned());
    }
    let mut selected = archive.clone();
    selected
        .entries
        .retain(|entry| categories.contains(&entry.category));
    selected
        .manifest
        .categories
        .retain(|category| categories.contains(category));

    let originals = selected
        .entries
        .iter()
        .map(|entry| read_original(root, &entry.name))
        .collect::<Result<Vec<_>, _>>()?;
    let plaintext = Zeroizing::new(
        serde_json::to_vec(&originals).map_err(|error| format!("序列化恢复点失败: {error}"))?,
    );
    let packet = encrypt_bytes(&plaintext, passphrase)?;
    let path = recovery_point_path(root)?;
    fs::create_dir_all(path.parent().ok_or("恢复点路径无效")?)
        .map_err(|error| format!("创建恢复点目录失败: {error}"))?;
    crate::atomic_file::write(&path, &packet)
        .map_err(|error| format!("保存恢复点失败: {error}"))?;

    if let Err(error) = restore_to(root, &selected) {
        return match write_originals(root, &originals) {
            Ok(()) => Err(error),
            Err(rollback) => Err(format!("{error}; {rollback}; 恢复点: {}", path.display())),
        };
    }
    Ok(RestorePoint {
        path,
        names: originals.into_iter().map(|file| file.name).collect(),
    })
}

/// 恢复点文件名 = 本机时间戳(纳秒,冲突再加序号),扩展名固定。
fn recovery_point_path(root: &Path) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    for sequence in 0..u32::MAX {
        let name = format!("backup-recovery/{stamp}-{sequence}.slterm-recovery");
        let path = restore_path(root, &name)?;
        if !path.exists() {
            return Ok(path);
        }
    }
    Err("无法分配恢复点文件名".to_owned())
}

/// 凭口令撤回:解密恢复点 → 目标名集合与句柄精确比对 → 写回原现场。
pub(crate) fn undo(point: &RestorePoint, passphrase: &str) -> Result<(), String> {
    undo_at(crate::platform::dirs::data_dir(), point, passphrase)
}

fn undo_at(root: &Path, point: &RestorePoint, passphrase: &str) -> Result<(), String> {
    let packet = fs::read(&point.path).map_err(|error| format!("读取恢复点失败: {error}"))?;
    let plaintext = decrypt_bytes(&packet, passphrase)?;
    let originals: Vec<OriginalFile> =
        serde_json::from_slice(&plaintext).map_err(|error| format!("恢复点内容无效: {error}"))?;
    // 只接受该次已验证恢复操作拥有的目标集合;外部导入文件不能借撤回扩展白名单。
    if originals
        .iter()
        .map(|file| &file.name)
        .ne(point.names.iter())
    {
        return Err("恢复点目标集合不匹配".to_owned());
    }
    write_originals(root, &originals)
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    const PASSPHRASE: &str = "correct horse";

    fn archive() -> BackupArchive {
        BackupArchive {
            manifest: BackupManifest {
                version: ARCHIVE_VERSION,
                categories: vec![BackupCategory::Settings, BackupCategory::Session],
            },
            entries: vec![
                BackupEntry {
                    category: BackupCategory::Settings,
                    name: "settings.json".into(),
                    bytes: br#"{"appearance":{"theme":"Nord"}}"#.to_vec(),
                },
                BackupEntry {
                    category: BackupCategory::Session,
                    name: "session.json".into(),
                    bytes: br#"{"version":1}"#.to_vec(),
                },
            ],
        }
    }

    #[test]
    fn selective_restore_preserves_other_categories_and_undo_restores_exact_bytes() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("settings.json"),
            br#"{"appearance":{"theme":"Linear"}}"#,
        )
        .unwrap();
        let point = restore_selected_at(
            root.path(),
            &archive(),
            BackupSelection::default(),
            PASSPHRASE,
        )
        .expect("选择性恢复");
        assert_eq!(
            fs::read(root.path().join("settings.json")).unwrap(),
            br#"{"appearance":{"theme":"Nord"}}"#
        );
        assert!(
            !root.path().join("session.json").exists(),
            "未选择的类别不得被恢复"
        );
        assert_eq!(point.names(), ["settings.json".to_owned()]);
        assert!(point.path().is_file(), "恢复点应已加密落盘");
        assert!(
            point
                .path()
                .starts_with(root.path().join("backup-recovery")),
            "恢复点落在本机 backup-recovery 目录"
        );

        assert!(undo_at(root.path(), &point, "wrong horse").is_err());
        undo_at(root.path(), &point, PASSPHRASE).expect("撤回");
        assert_eq!(
            fs::read(root.path().join("settings.json")).unwrap(),
            br#"{"appearance":{"theme":"Linear"}}"#
        );

        // 恢复前不存在 → 撤回把恢复新建的文件删掉。
        let point = restore_selected_at(
            root.path(),
            &archive(),
            BackupSelection::from_categories([BackupCategory::Session]),
            PASSPHRASE,
        )
        .expect("会话恢复");
        assert!(root.path().join("session.json").exists());
        undo_at(root.path(), &point, PASSPHRASE).expect("撤回会话");
        assert!(!root.path().join("session.json").exists());
    }

    #[test]
    fn recovery_point_is_encrypted_and_never_carries_plaintext() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("settings.json"),
            br#"{"appearance":{"theme":"Linear-SECRET-MARKER"}}"#,
        )
        .unwrap();
        let point = restore_selected_at(
            root.path(),
            &archive(),
            BackupSelection::default(),
            PASSPHRASE,
        )
        .expect("选择性恢复");
        let packet = fs::read(point.path()).unwrap();
        assert_eq!(&packet[..MAGIC.len()], MAGIC, "恢复点是加密报文");
        assert!(
            !packet
                .windows("Linear-SECRET-MARKER".len())
                .any(|window| window == b"Linear-SECRET-MARKER"),
            "恢复点不得以明文携带本机原值"
        );
    }

    #[test]
    fn undo_rejects_recovery_points_with_a_different_target_set() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("settings.json"), b"local").unwrap();
        let point = restore_selected_at(
            root.path(),
            &archive(),
            BackupSelection::default(),
            PASSPHRASE,
        )
        .expect("选择性恢复");

        // 伪造一份目标集合不同的恢复点文件:撤回必须拒绝,不能借机扩白名单。
        let forged_originals = vec![
            OriginalFile {
                name: "settings.json".into(),
                bytes: Some(b"forged".to_vec()),
            },
            OriginalFile {
                name: "session.json".into(),
                bytes: None,
            },
        ];
        let plaintext = serde_json::to_vec(&forged_originals).unwrap();
        let packet = encrypt_bytes(&plaintext, PASSPHRASE).unwrap();
        let forged = RestorePoint {
            path: point.path.clone(),
            names: point.names.clone(),
        };
        fs::write(&forged.path, packet).unwrap();
        let before = fs::read(root.path().join("settings.json")).unwrap();
        assert!(undo_at(root.path(), &forged, PASSPHRASE).is_err());
        assert_eq!(fs::read(root.path().join("settings.json")).unwrap(), before);
        assert!(!root.path().join("session.json").exists());
    }

    #[test]
    fn failed_restore_rolls_back_originals() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("settings.json"), b"original").unwrap();
        // 第 0 条写成功后注入失败:第 1 条不再写,回滚必须恢复第 0 条、
        // 且不留下第 1 条(恢复前不存在)。
        *super::super::RESTORE_FAILPOINT.lock().unwrap() = Some("session.json".to_owned());
        let result = restore_selected_at(
            root.path(),
            &archive(),
            BackupSelection::from_categories([BackupCategory::Settings, BackupCategory::Session]),
            PASSPHRASE,
        );
        *super::super::RESTORE_FAILPOINT.lock().unwrap() = None;
        assert!(result.is_err(), "故障注入必须向上传播");
        assert_eq!(
            fs::read(root.path().join("settings.json")).unwrap(),
            b"original",
            "失败恢复必须回滚已写文件"
        );
        assert!(
            !root.path().join("session.json").exists(),
            "失败恢复不得留下恢复新建的文件"
        );
        assert!(
            root.path().join("backup-recovery").is_dir(),
            "恢复点仍保留供人工诊断"
        );
    }

    #[test]
    fn restore_selected_rejects_empty_selection() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            restore_selected_at(
                root.path(),
                &archive(),
                BackupSelection::from_categories([]),
                PASSPHRASE
            )
            .is_err()
        );
    }
}
