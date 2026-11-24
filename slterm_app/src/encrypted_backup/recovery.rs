//! 选择性恢复与其加密恢复点:UI 只持句柄,不承担文件
//! 写入规则。
//!
//! 恢复点只在本机加密落盘,不经会过滤敏感字段的可分享收集器;撤回时类别与
//! 目标名集合必须与该次已验证恢复完全一致,外部导入文件不能借撤回扩展白名单。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest as _, Sha256};

use super::*;

/// 恢复目标的类别与根内相对名共同构成身份，禁止跨根伪造。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RestoreTarget {
    category: BackupCategory,
    name: String,
}

/// 一次选择性恢复之前的本机现场句柄。
#[derive(Clone, Debug)]
pub(crate) struct RestorePoint {
    pub(crate) path: PathBuf,
    /// 目标集合私有，写入规则不暴露给 UI。
    targets: Vec<RestoreTarget>,
}

impl RestorePoint {
    /// 恢复点文件路径(UI 展示/清理用)。
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Serialize, Deserialize)]
struct OriginalFile {
    target: RestoreTarget,
    /// None = 恢复前该文件不存在;撤回时删除,而不是留下恢复新建的文件。
    bytes: Option<Vec<u8>>,
    /// 恢复成功后的文件指纹；撤回时逐项比较，后续修改项保留当前内容。
    #[serde(default)]
    after: Option<[u8; 32]>,
}

fn file_fingerprint(path: &Path) -> Result<Option<[u8; 32]>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(Sha256::digest(&bytes).into())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("读取撤回目标指纹失败: {error}")),
    }
}

fn read_original(paths: &AppPaths, entry: &BackupEntry) -> Result<OriginalFile, String> {
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

fn write_originals(paths: &AppPaths, originals: &[OriginalFile]) -> Result<(), String> {
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

/// 选择性恢复:先采原字节与「不存在」状态 → DPAPI 本机加密落盘恢复点 →
/// 原子写恢复;恢复失败自动回滚。恢复点不要求再次输入口令。
pub(crate) fn restore_selected(
    archive: &BackupArchive,
    selection: BackupSelection,
) -> Result<RestorePoint, String> {
    restore_selected_at(crate::platform::dirs::app_paths(), archive, selection)
}

fn restore_selected_at(
    paths: &AppPaths,
    archive: &BackupArchive,
    selection: BackupSelection,
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

    let mut originals = selected
        .entries
        .iter()
        .map(|entry| read_original(paths, entry))
        .collect::<Result<Vec<_>, _>>()?;
    let plaintext = Zeroizing::new(
        serde_json::to_vec(&originals).map_err(|error| format!("序列化恢复点失败: {error}"))?,
    );
    let packet = crate::platform::dpapi::protect(&plaintext)
        .map_err(|error| format!("恢复点 DPAPI 加密失败: {error}"))?;
    let path = recovery_point_path(paths.local_root())?;
    fs::create_dir_all(path.parent().ok_or("恢复点路径无效")?)
        .map_err(|error| format!("创建恢复点目录失败: {error}"))?;
    crate::atomic_file::write(&path, &packet)
        .map_err(|error| format!("保存恢复点失败: {error}"))?;

    if let Err(error) = restore_to(paths, &selected) {
        return match write_originals(paths, &originals) {
            Ok(()) => Err(error),
            Err(rollback) => Err(format!("{error}; {rollback}; 恢复点: {}", path.display())),
        };
    }
    // 恢复成功后补记每项恢复后指纹：撤回只回滚未被后续修改的文件。
    for file in originals.iter_mut() {
        let target = restore_path(
            category_root(paths, file.target.category),
            &file.target.name,
        )?;
        file.after = file_fingerprint(&target)?;
    }
    let plaintext = Zeroizing::new(
        serde_json::to_vec(&originals).map_err(|error| format!("序列化恢复点失败: {error}"))?,
    );
    let packet = crate::platform::dpapi::protect(&plaintext)
        .map_err(|error| format!("恢复点 DPAPI 加密失败: {error}"))?;
    crate::atomic_file::write(&path, &packet)
        .map_err(|error| format!("更新恢复点失败: {error}"))?;
    Ok(RestorePoint {
        path,
        targets: originals.into_iter().map(|file| file.target).collect(),
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

/// 撤回:DPAPI 解密恢复点 → 目标名集合与句柄精确比对 → 逐项指纹检查 →
/// 写回未被后续修改的原现场；被修改项标冲突并保留当前内容。
pub(crate) fn undo(point: &RestorePoint) -> Result<(), String> {
    undo_at(crate::platform::dirs::app_paths(), point)
}

fn undo_at(paths: &AppPaths, point: &RestorePoint) -> Result<(), String> {
    let packet = fs::read(&point.path).map_err(|error| format!("读取恢复点失败: {error}"))?;
    let plaintext = crate::platform::dpapi::unprotect(&packet)
        .map_err(|error| format!("恢复点 DPAPI 解密失败: {error}"))?;
    let originals: Vec<OriginalFile> =
        serde_json::from_slice(&plaintext).map_err(|error| format!("恢复点内容无效: {error}"))?;
    // 只接受该次已验证恢复操作拥有的目标集合;外部导入文件不能借撤回扩展白名单。
    if originals
        .iter()
        .map(|file| &file.target)
        .ne(point.targets.iter())
    {
        return Err("恢复点目标集合不匹配".to_owned());
    }
    let mut matched = Vec::new();
    let mut conflicts = Vec::new();
    for file in originals {
        let target = restore_path(
            category_root(paths, file.target.category),
            &file.target.name,
        )?;
        let current = file_fingerprint(&target)?;
        if current == file.after {
            matched.push(file);
        } else if current
            == file
                .bytes
                .as_deref()
                .map(|bytes| Sha256::digest(bytes).into())
        {
            // 已是撤回前现场（重复撤回）：不需要再次写盘，也不构成冲突。
        } else {
            conflicts.push(file.target.name);
        }
    }
    write_originals(paths, &matched)?;
    if !conflicts.is_empty() {
        return Err(format!(
            "撤回冲突：以下文件在恢复后被修改，已保留当前内容: {}",
            conflicts.join(", ")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    const PASSPHRASE: &str = "correct horse";

    fn single_root_paths(root: &Path) -> AppPaths {
        AppPaths::new(root, root).unwrap()
    }

    fn split_paths(root: &Path) -> AppPaths {
        AppPaths::new(root.join("config"), root.join("local")).unwrap()
    }

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

    /// 端到端：收集 → 密封 → 打开 → 选择性恢复 → 撤回恢复前现场。
    #[test]
    fn integration_encrypted_backup() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        let settings = br#"{"appearance":{"theme":"Linear"}}"#.to_vec();
        fs::write(paths.config_root().join("settings.json"), &settings).unwrap();
        fs::write(paths.local_root().join("session.json"), br#"{"version":0}"#).unwrap();
        fs::create_dir_all(paths.config_root().join("themes")).unwrap();
        let theme = crate::theme_library::builtin_documents()
            .first()
            .expect("内置主题文档非空")
            .to_json_bytes()
            .expect("内置主题文档可序列化");
        let theme_path = paths.config_root().join("themes/copy.slterm-theme.json");
        fs::write(&theme_path, &theme).unwrap();

        let archive = collect_from(
            &paths,
            BackupSelection::from_categories([
                BackupCategory::Settings,
                BackupCategory::Session,
                BackupCategory::Themes,
            ]),
        )
        .unwrap();
        let packet = seal(&archive, PASSPHRASE).unwrap();
        let opened = open(&packet, PASSPHRASE).unwrap();
        assert_eq!(opened, archive);

        fs::write(
            paths.config_root().join("settings.json"),
            br#"{"appearance":{"theme":"Nord"}}"#,
        )
        .unwrap();
        fs::write(&theme_path, b"external").unwrap();
        let point = restore_selected_at(
            &paths,
            &opened,
            BackupSelection::from_categories([
                BackupCategory::Settings,
                BackupCategory::Session,
                BackupCategory::Themes,
            ]),
        )
        .unwrap();
        assert_eq!(
            fs::read(paths.config_root().join("settings.json")).unwrap(),
            settings
        );
        assert_eq!(fs::read(&theme_path).unwrap(), theme);
        assert_eq!(
            fs::read(paths.local_root().join("session.json")).unwrap(),
            br#"{"version":0}"#
        );

        undo_at(&paths, &point).unwrap();
        assert_eq!(
            fs::read(paths.config_root().join("settings.json")).unwrap(),
            br#"{"appearance":{"theme":"Nord"}}"#,
            "撤回必须回到恢复前现场"
        );
        assert_eq!(fs::read(&theme_path).unwrap(), b"external");
    }

    /// 撤回逐项检查恢复后指纹：外部修改项保留当前并报告冲突，其余项回滚。
    #[test]
    fn backup_undo_fingerprint_conflict() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(
            paths.settings_path(),
            br#"{"appearance":{"theme":"Linear"}}"#,
        )
        .unwrap();
        fs::write(paths.local_root().join("session.json"), br#"{"version":0}"#).unwrap();

        let point = restore_selected_at(
            &paths,
            &archive(),
            BackupSelection::from_categories([BackupCategory::Settings, BackupCategory::Session]),
        )
        .unwrap();
        fs::write(
            paths.settings_path(),
            br#"{"appearance":{"theme":"MintDark"}}"#,
        )
        .unwrap();

        let error = undo_at(&paths, &point).unwrap_err();
        assert!(error.contains("冲突"), "撤回必须报告指纹冲突: {error}");
        assert!(error.contains("settings.json"), "冲突项必须点名: {error}");
        assert_eq!(
            fs::read(paths.settings_path()).unwrap(),
            br#"{"appearance":{"theme":"MintDark"}}"#,
            "冲突项必须保留当前内容"
        );
        assert_eq!(
            fs::read(paths.local_root().join("session.json")).unwrap(),
            br#"{"version":0}"#,
            "未修改项必须回滚"
        );

        fs::write(paths.settings_path(), br#"{"appearance":{"theme":"Nord"}}"#).unwrap();
        undo_at(&paths, &point).expect("指纹恢复一致后可撤回");
        assert_eq!(
            fs::read(paths.settings_path()).unwrap(),
            br#"{"appearance":{"theme":"Linear"}}"#
        );
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
            &single_root_paths(root.path()),
            &archive(),
            BackupSelection::default(),
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
        assert_eq!(
            point.targets,
            [RestoreTarget {
                category: BackupCategory::Settings,
                name: "settings.json".to_owned(),
            }]
        );
        assert!(point.path().is_file(), "恢复点应已加密落盘");
        assert!(
            point
                .path()
                .starts_with(root.path().join("backup-recovery")),
            "恢复点落在本机 backup-recovery 目录"
        );

        // DPAPI 密文被篡改后撤回必须失败且零写入。
        let packet = fs::read(point.path()).unwrap();
        let mut tampered = packet.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        fs::write(point.path(), tampered).unwrap();
        assert!(undo_at(&single_root_paths(root.path()), &point).is_err());
        fs::write(point.path(), packet).unwrap();
        undo_at(&single_root_paths(root.path()), &point).expect("撤回");
        assert_eq!(
            fs::read(root.path().join("settings.json")).unwrap(),
            br#"{"appearance":{"theme":"Linear"}}"#
        );

        // 恢复前不存在 → 撤回把恢复新建的文件删掉。
        let point = restore_selected_at(
            &single_root_paths(root.path()),
            &archive(),
            BackupSelection::from_categories([BackupCategory::Session]),
        )
        .expect("会话恢复");
        assert!(root.path().join("session.json").exists());
        undo_at(&single_root_paths(root.path()), &point).expect("撤回会话");
        assert!(!root.path().join("session.json").exists());
    }

    #[test]
    fn selective_restore_and_undo_scope_originals_across_split_roots() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(
            paths.settings_path(),
            br#"{"appearance":{"theme":"Linear"}}"#,
        )
        .unwrap();
        fs::write(paths.local_root().join("session.json"), br#"{"version":0}"#).unwrap();
        fs::write(paths.local_root().join("settings.json"), b"local-decoy").unwrap();
        fs::write(paths.config_root().join("session.json"), b"config-decoy").unwrap();

        let point = restore_selected_at(
            &paths,
            &archive(),
            BackupSelection::from_categories([BackupCategory::Settings, BackupCategory::Session]),
        )
        .unwrap();
        assert_eq!(
            fs::read(paths.settings_path()).unwrap(),
            br#"{"appearance":{"theme":"Nord"}}"#
        );
        assert_eq!(
            fs::read(paths.local_root().join("session.json")).unwrap(),
            br#"{"version":1}"#
        );
        assert_eq!(
            fs::read(paths.local_root().join("settings.json")).unwrap(),
            b"local-decoy"
        );
        assert_eq!(
            fs::read(paths.config_root().join("session.json")).unwrap(),
            b"config-decoy"
        );
        assert!(
            point
                .path()
                .starts_with(paths.local_root().join("backup-recovery")),
            "恢复点必须始终归本地根"
        );

        undo_at(&paths, &point).unwrap();
        assert_eq!(
            fs::read(paths.settings_path()).unwrap(),
            br#"{"appearance":{"theme":"Linear"}}"#
        );
        assert_eq!(
            fs::read(paths.local_root().join("session.json")).unwrap(),
            br#"{"version":0}"#
        );
        assert_eq!(
            fs::read(paths.local_root().join("settings.json")).unwrap(),
            b"local-decoy"
        );
        assert_eq!(
            fs::read(paths.config_root().join("session.json")).unwrap(),
            b"config-decoy"
        );
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
            &single_root_paths(root.path()),
            &archive(),
            BackupSelection::default(),
        )
        .expect("选择性恢复");
        let packet = fs::read(point.path()).unwrap();
        // DPAPI 报文无固定魔数；加密性由「不含明文 + 篡改拒绝」两条断言锁定。
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
            &single_root_paths(root.path()),
            &archive(),
            BackupSelection::default(),
        )
        .expect("选择性恢复");

        // 伪造一份目标集合不同的恢复点文件:撤回必须拒绝,不能借机扩白名单。
        let forged_originals = vec![
            OriginalFile {
                target: RestoreTarget {
                    category: BackupCategory::Settings,
                    name: "settings.json".into(),
                },
                bytes: Some(b"forged".to_vec()),
                after: None,
            },
            OriginalFile {
                target: RestoreTarget {
                    category: BackupCategory::Session,
                    name: "session.json".into(),
                },
                bytes: None,
                after: None,
            },
        ];
        let plaintext = serde_json::to_vec(&forged_originals).unwrap();
        let packet = crate::platform::dpapi::protect(&plaintext).unwrap();
        let forged = RestorePoint {
            path: point.path.clone(),
            targets: point.targets.clone(),
        };
        fs::write(&forged.path, packet).unwrap();
        let before = fs::read(root.path().join("settings.json")).unwrap();
        assert!(undo_at(&single_root_paths(root.path()), &forged).is_err());
        assert_eq!(fs::read(root.path().join("settings.json")).unwrap(), before);
        assert!(!root.path().join("session.json").exists());
    }

    #[test]
    fn undo_rejects_same_name_with_a_forged_category() {
        let directory = tempfile::tempdir().unwrap();
        let paths = split_paths(directory.path());
        fs::create_dir_all(paths.config_root()).unwrap();
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(
            paths.settings_path(),
            br#"{"appearance":{"theme":"Linear"}}"#,
        )
        .unwrap();
        let point = restore_selected_at(&paths, &archive(), BackupSelection::default()).unwrap();

        // 文件名与句柄一致，仅类别被伪造；完整目标比对必须拒绝跨根写入。
        let forged_originals = vec![OriginalFile {
            target: RestoreTarget {
                category: BackupCategory::Session,
                name: "settings.json".into(),
            },
            bytes: Some(b"forged".to_vec()),
            after: None,
        }];
        let packet =
            crate::platform::dpapi::protect(&serde_json::to_vec(&forged_originals).unwrap())
                .unwrap();
        fs::write(point.path(), packet).unwrap();
        assert!(undo_at(&paths, &point).is_err());
        assert_eq!(
            fs::read(paths.settings_path()).unwrap(),
            br#"{"appearance":{"theme":"Nord"}}"#
        );
        assert!(!paths.local_root().join("settings.json").exists());
    }

    #[test]
    fn backup_interrupted_restore_rolls_back() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("settings.json"), b"original").unwrap();
        // 第 0 条写成功后注入失败:第 1 条不再写,回滚必须恢复第 0 条、
        // 且不留下第 1 条(恢复前不存在)。
        *super::super::RESTORE_FAILPOINT.lock().unwrap() = Some("session.json".to_owned());
        let result = restore_selected_at(
            &single_root_paths(root.path()),
            &archive(),
            BackupSelection::from_categories([BackupCategory::Settings, BackupCategory::Session]),
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
                &single_root_paths(root.path()),
                &archive(),
                BackupSelection::from_categories([]),
            )
            .is_err()
        );
    }
}
