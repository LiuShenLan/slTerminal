//! settings.json 读写适配层(06 篇 M6.1):写通道 = 白名单校验 → 段级浅合并
//! → 原子写 → .bak;读侧 = 缺失/损坏/正常三态。
//!
//! 分层:值模型与键注册表归 `slterm-settings`(零生产依赖);保存锁、.bak、
//! 损坏 quarantine 等硬机制只在本层。仓内唯一设置写入口 =
//! [`save_settings_blocking`](M1 过渡的裸 `persist_keys` 已由本模块取代,
//! 契约语义不变:读-改-写浅合并、未知段保留、损坏禁覆盖、1MB 上限)。
//!
//! 损坏三态(与 session 同纪律:读侧回退、写侧拒绝):
//! - 缺失 → 默认值,不标记;
//! - 损坏 → 原文件 quarantine 为 `settings.corrupted.json`,再尝试 .bak 重建;
//! - 正常 → 读取返回。
//!
//! 损坏文件绝不原地修复式覆盖:先隔离保留诊断材料,重建只发生在损坏件
//! 已移走之后;隔离失败则保留原文件、拒绝重建。

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use slterm_settings::json::{self, Value};
use slterm_settings::{MAX_PERSIST_BYTES, RawSettings, keys, settings_path};

/// settings.json 读侧三态结果。`corrupted = true` 表示数据来自 .bak 或
/// 默认回退(损坏原件已隔离),设置页据此显示警示条(M6.2 消费)。
#[allow(dead_code)] // 消费点:06 篇 M6.2 设置页启动读取与损坏警示条。
pub(crate) struct LoadResult {
    pub(crate) data: RawSettings,
    pub(crate) corrupted: bool,
}

/// 保存锁:串行化「读-合并-写」全程,消除并发 persist 的 Windows 句柄
/// 占用冲突(杀软实时扫描窗口为残余偶发源)。锁内 panic 后的毒化不扩大
/// 为进程级故障——后续写入继续持锁执行。
static SAVE_LOCK: Mutex<()> = Mutex::new(());

/// 读取设置(生产入口)。IO 错误向上传播,不把权限/磁盘故障当默认值。
#[allow(dead_code)] // 消费点:06 篇 M6.2 设置页启动读取。
pub(crate) fn load_settings() -> io::Result<LoadResult> {
    load_settings_at(&settings_path())
}

/// 显式路径读入口(测试注入与工作区导入共用)。
#[allow(dead_code)] // 测试注入路径;生产入口见 load_settings。
pub(crate) fn load_settings_at(path: &Path) -> io::Result<LoadResult> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(LoadResult {
                data: RawSettings::default(),
                corrupted: false,
            });
        }
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!("读取设置失败: {error}"),
            ));
        }
    };
    match json::parse(&bytes) {
        Ok(Value::Object(_)) => Ok(LoadResult {
            data: RawSettings::from_json_bytes(&bytes),
            corrupted: false,
        }),
        // 解析失败或根非对象都算损坏:设置文档的根形态是对象。
        Ok(_) | Err(_) => Ok(recover_corrupted(path)),
    }
}

/// 写通道(生产入口):更新项为注册表 leaf 键或域外段名。
pub(crate) fn save_settings_blocking(updates: &[(&str, Value)]) -> io::Result<()> {
    save_settings_blocking_at(&settings_path(), updates)
}

/// 显式路径写入口:校验 → 保存锁 → 读现有 → 段级浅合并 → 大小上限 →
/// .bak 备份 → 临时文件 + 原子替换。
#[allow(dead_code)] // 测试注入路径;生产入口见 save_settings_blocking。
pub(crate) fn save_settings_blocking_at(path: &Path, updates: &[(&str, Value)]) -> io::Result<()> {
    validate_updates(updates)?;
    let _guard = SAVE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let existing = read_existing(path)?;
    let merged = slterm_settings::apply_updates(&existing, updates);
    let text = json::to_string_pretty(&merged);
    if text.len() > MAX_PERSIST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "保存设置失败: 数据超过 1MB 上限",
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // 备份兜底:旧文件 copy 为 .bak,失败只告警——备份不阻断主写入。
    if path.is_file()
        && let Err(error) = std::fs::copy(path, backup_path(path))
    {
        log::warn!("settings .bak 备份失败: {error}");
    }
    crate::atomic_file::write(path, text.as_bytes())
}

/// 白名单校验:更新键必须是注册表 leaf 键或域外段名;未知键拒绝落盘。
fn validate_updates(updates: &[(&str, Value)]) -> io::Result<()> {
    for (key, _) in updates {
        if keys::lookup(key).is_some() || keys::is_external_segment(key) {
            continue;
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("保存设置失败: 键不在白名单内: {key}"),
        ));
    }
    Ok(())
}

/// 写通道读法:缺失 = 首次启动(空树);IO/解析/根形态错误一律 Err 传播,
/// 损坏文件不会被 incoming 整体覆盖。
fn read_existing(path: &Path) -> io::Result<Value> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Value::empty_object()),
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!("读取设置失败: {error}"),
            ));
        }
    };
    match json::parse(&bytes) {
        Ok(tree @ Value::Object(_)) => Ok(tree),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("读取设置失败: {} 根不是对象, 保存中止", path.display()),
        )),
        Err(error) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("读取设置失败: {} 损坏, 保存中止: {error}", path.display()),
        )),
    }
}

/// 损坏恢复:隔离原件 → 尝试 .bak 重建 → 失败回默认。
/// 隔离失败时保留原文件并拒绝重建(损坏禁覆盖)。
fn recover_corrupted(path: &Path) -> LoadResult {
    if quarantine(path).is_none() {
        log::warn!("settings.json 损坏且隔离失败;保留原文件, 不做重建");
        return LoadResult {
            data: RawSettings::default(),
            corrupted: true,
        };
    }
    let Some(backup) = read_backup_tree(path) else {
        return LoadResult {
            data: RawSettings::default(),
            corrupted: true,
        };
    };
    // 备份数据重整为 pretty 形态重建主文件;损坏原件已在 quarantine 中保留。
    let text = json::to_string_pretty(&backup);
    if let Err(error) = crate::atomic_file::write(path, text.as_bytes()) {
        log::warn!("settings.json 从 .bak 重建失败: {error}");
    }
    LoadResult {
        data: RawSettings::from_json_bytes(text.as_bytes()),
        corrupted: true,
    }
}

/// 隔离损坏文件:`settings.json` → `settings.corrupted.json`(copy + remove,
/// 与 session 的 `session.crashed.json` 同形态)。返回隔离文件路径。
fn quarantine(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_str()?;
    let target = path.with_file_name(format!("{stem}.corrupted.json"));
    std::fs::copy(path, &target).ok()?;
    std::fs::remove_file(path).ok()?;
    Some(target)
}

/// .bak 可用的判据 = 可读且解析为对象;双损时返回 None。
fn read_backup_tree(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(backup_path(path)).ok()?;
    match json::parse(&bytes) {
        Ok(tree @ Value::Object(_)) => Some(tree),
        Ok(_) | Err(_) => None,
    }
}

/// 同名 `.bak` 路径(与旧便携写通道同名,便于手工排障)。
fn backup_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

#[cfg(test)]
mod settings_store_tests {
    use super::*;
    use slterm_settings::{AgentHook, BellModeName, RuntimeSettings, ThemeName};

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        (directory, path)
    }

    fn write_json(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    fn read_tree(path: &Path) -> Value {
        json::parse(&std::fs::read(path).unwrap()).unwrap()
    }

    // ---- 写通道:校验/浅合并/原子写/.bak ----

    #[test]
    fn validation_rejects_unknown_keys_without_touching_disk() {
        let (directory, path) = fixture();
        let error = save_settings_blocking_at(&path, &[("handmade_section", Value::Bool(true))])
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("不在白名单内"));
        assert!(!path.exists(), "校验失败不得落盘");
        assert!(
            !directory.path().join("settings.json.bak").exists(),
            "校验失败不得产生 .bak"
        );
    }

    #[test]
    fn writes_merge_per_segment_and_preserve_unknown_sections() {
        let (_directory, path) = fixture();
        write_json(
            &path,
            "{\n  \"appearance\": {\n    \"theme\": \"Nord\",\n    \"opacity\": 0.9\n  },\n  \"handmade\": {\n    \"note\": \"keep\"\n  }\n}\n",
        );
        save_settings_blocking_at(
            &path,
            &[
                ("font_size", Value::Number(16.0)),
                ("scrollback_lines", Value::Number(50000.0)),
            ],
        )
        .unwrap();
        let tree = read_tree(&path);
        let appearance = tree.get("appearance").unwrap();
        assert_eq!(appearance.get("theme").unwrap().as_str(), Some("Nord"));
        assert_eq!(appearance.get("opacity").unwrap().as_f64(), Some(0.9));
        assert_eq!(appearance.get("font_size").unwrap().as_f64(), Some(16.0));
        assert_eq!(
            tree.get("scrolling")
                .unwrap()
                .get("scrollback_lines")
                .unwrap()
                .as_f64(),
            Some(50000.0)
        );
        assert_eq!(
            tree.get("handmade").unwrap().get("note").unwrap().as_str(),
            Some("keep"),
            "未知段必须原样保留"
        );
    }

    #[test]
    fn atomic_write_leaves_only_the_document_and_its_backup() {
        let (directory, path) = fixture();
        save_settings_blocking_at(&path, &[("font_size", Value::Number(14.0))]).unwrap();
        save_settings_blocking_at(&path, &[("theme", Value::String("Nord".into()))]).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["settings.json", "settings.json.bak"],
            "原子替换不得残留临时文件"
        );
    }

    #[test]
    fn backup_copies_the_previous_document_before_replace() {
        let (_directory, path) = fixture();
        save_settings_blocking_at(&path, &[("theme", Value::String("Nord".into()))]).unwrap();
        let first = std::fs::read(&path).unwrap();
        save_settings_blocking_at(&path, &[("font_size", Value::Number(16.0))]).unwrap();
        assert_eq!(
            std::fs::read(backup_path(&path)).unwrap(),
            first,
            ".bak 应是替换前的完整旧文档"
        );
    }

    #[test]
    fn corrupt_existing_document_is_rejected_and_left_untouched() {
        let (_directory, path) = fixture();
        let corrupt = "{ not json";
        write_json(&path, corrupt);
        let error = save_settings_blocking_at(&path, &[("theme", Value::String("Nord".into()))])
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("损坏"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), corrupt);
        assert!(!backup_path(&path).exists(), "保存中止不得产生 .bak");
    }

    #[test]
    fn non_object_root_is_treated_as_corrupt_on_write() {
        let (_directory, path) = fixture();
        write_json(&path, "[1, 2, 3]");
        let error = save_settings_blocking_at(&path, &[("theme", Value::String("Nord".into()))])
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[1, 2, 3]");
    }

    #[test]
    fn oversized_documents_are_rejected_without_writing() {
        let (_directory, path) = fixture();
        let huge = "x".repeat(MAX_PERSIST_BYTES);
        let error =
            save_settings_blocking_at(&path, &[("custom_theme", Value::String(huge))]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
        assert!(!path.exists());
    }

    #[test]
    fn external_segments_are_accepted_and_replaced_whole() {
        let (_directory, path) = fixture();
        let first = Value::Array(vec![Value::String("ctrl+shift+c".into())]);
        let second = Value::Array(vec![Value::String("ctrl+x".into())]);
        let modes = Value::Object(vec![("inherit_cursor".into(), Value::Bool(true))]);
        save_settings_blocking_at(&path, &[(keys::KEYBIND_SEGMENT, first)]).unwrap();
        save_settings_blocking_at(
            &path,
            &[
                (keys::KEYBIND_SEGMENT, second.clone()),
                (keys::CONPTY_INPUT_MODES_SEGMENT, modes.clone()),
            ],
        )
        .unwrap();
        let tree = read_tree(&path);
        assert_eq!(
            tree.get(keys::KEYBIND_SEGMENT),
            Some(&second),
            "07 篇 keybind 段整表替换,不与旧数组浅合并"
        );
        assert_eq!(tree.get(keys::CONPTY_INPUT_MODES_SEGMENT), Some(&modes));
    }

    #[test]
    fn concurrent_writers_are_serialized_and_keep_both_updates() {
        let (_directory, path) = fixture();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = [
            ("theme", Value::String("Nord".into())),
            ("scroll_speed", Value::Number(2.0)),
        ]
        .into_iter()
        .map(|(key, value)| {
            let path = path.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                save_settings_blocking_at(&path, &[(key, value)])
            })
        })
        .collect();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        let tree = read_tree(&path);
        assert_eq!(
            tree.get("appearance")
                .unwrap()
                .get("theme")
                .unwrap()
                .as_str(),
            Some("Nord")
        );
        assert_eq!(
            tree.get("scrolling")
                .unwrap()
                .get("scroll_speed")
                .unwrap()
                .as_f64(),
            Some(2.0),
            "保存锁应串行化读-合并-写,两笔更新都保留"
        );
    }

    #[test]
    fn saved_values_round_trip_through_runtime_settings() {
        let (_directory, path) = fixture();
        save_settings_blocking_at(
            &path,
            &[
                ("theme", Value::String("Nord".into())),
                ("font_size", Value::Number(16.5)),
                ("scrollback_lines", Value::Number(50000.0)),
                ("bell", Value::String("audible".into())),
                ("restore_session", Value::Bool(false)),
                ("slterm_ai_hooks_grok", Value::Bool(true)),
            ],
        )
        .unwrap();
        save_settings_blocking_at(&path, &[("opacity", Value::Number(0.9))]).unwrap();

        let raw = RawSettings::from_json_bytes(&std::fs::read(&path).unwrap());
        let settings = RuntimeSettings::from_raw(&raw);
        assert_eq!(settings.theme, ThemeName::Nord);
        assert_eq!(settings.font_size_px, Some(16.5));
        assert_eq!(settings.scrollback_lines, 50_000);
        assert_eq!(settings.bell, BellModeName::Audible);
        assert!(!settings.restore_session);
        assert_eq!(settings.opacity, 0.9);
        assert!(settings.opacity_is_explicit());
        assert!(AgentHook::Grok.enabled(&raw));
    }

    // ---- 读侧三态:缺失/正常/损坏 quarantine+重建 ----

    #[test]
    fn missing_file_reads_defaults_without_corruption_flag() {
        let (_directory, path) = fixture();
        let loaded = load_settings_at(&path).unwrap();
        assert!(!loaded.corrupted);
        assert_eq!(loaded.data.keys().count(), 0);
        assert!(!path.exists(), "读取不得凭空创建文件");
    }

    #[test]
    fn valid_file_reads_without_corruption_flag() {
        let (_directory, path) = fixture();
        save_settings_blocking_at(
            &path,
            &[
                ("theme", Value::String("Nord".into())),
                ("restore_session", Value::Bool(false)),
            ],
        )
        .unwrap();
        let loaded = load_settings_at(&path).unwrap();
        assert!(!loaded.corrupted);
        let settings = RuntimeSettings::from_raw(&loaded.data);
        assert_eq!(settings.theme, ThemeName::Nord);
        assert!(!settings.restore_session);
    }

    #[test]
    fn corrupt_file_is_quarantined_and_rebuilt_from_backup() {
        let (_directory, path) = fixture();
        let corrupt = "{ not json";
        write_json(&path, corrupt);
        write_json(
            &backup_path(&path),
            "{\n  \"appearance\": {\n    \"theme\": \"Nord\"\n  }\n}\n",
        );
        let loaded = load_settings_at(&path).unwrap();
        assert!(loaded.corrupted, ".bak 命中必须标记 corrupted");
        assert_eq!(
            RuntimeSettings::from_raw(&loaded.data).theme,
            ThemeName::Nord
        );
        // 损坏原件完整保留在隔离文件里。
        let quarantined = path.with_file_name("settings.corrupted.json");
        assert_eq!(std::fs::read_to_string(&quarantined).unwrap(), corrupt);
        // 主文件已从 .bak 重建(非原地覆盖损坏件)。
        let rebuilt = read_tree(&path);
        assert_eq!(
            rebuilt
                .get("appearance")
                .unwrap()
                .get("theme")
                .unwrap()
                .as_str(),
            Some("Nord")
        );
    }

    #[test]
    fn corrupt_file_without_backup_reports_defaults_and_keeps_quarantine() {
        let (_directory, path) = fixture();
        let corrupt = "definitely not json {{{";
        write_json(&path, corrupt);
        let loaded = load_settings_at(&path).unwrap();
        assert!(loaded.corrupted);
        assert_eq!(loaded.data.keys().count(), 0, "无 .bak 时回默认");
        assert_eq!(
            std::fs::read_to_string(path.with_file_name("settings.corrupted.json")).unwrap(),
            corrupt
        );
        assert!(!path.exists(), "损坏件已隔离,主路径待下次写入重建");
    }

    #[test]
    fn double_corruption_reports_corruption_and_keeps_both_originals() {
        let (_directory, path) = fixture();
        write_json(&path, "not json {{{");
        write_json(&backup_path(&path), "also corrupt ###");
        let loaded = load_settings_at(&path).unwrap();
        assert!(loaded.corrupted);
        assert_eq!(loaded.data.keys().count(), 0);
        assert_eq!(
            std::fs::read_to_string(path.with_file_name("settings.corrupted.json")).unwrap(),
            "not json {{{"
        );
        assert_eq!(
            std::fs::read_to_string(backup_path(&path)).unwrap(),
            "also corrupt ###",
            "不可用 .bak 不删除,留作诊断"
        );
    }

    #[test]
    fn parseable_non_object_root_is_treated_as_corrupted() {
        let (_directory, path) = fixture();
        write_json(&path, "[1, 2, 3]");
        let loaded = load_settings_at(&path).unwrap();
        assert!(loaded.corrupted);
        assert_eq!(loaded.data.keys().count(), 0);
        assert!(path.with_file_name("settings.corrupted.json").exists());
    }

    #[test]
    fn read_io_errors_are_surfaced_instead_of_defaulting() {
        let (directory, path) = fixture();
        std::fs::create_dir(&path).unwrap();
        assert!(load_settings_at(&path).is_err());
        // 写通道同样拒绝:读失败不得吞成空树覆盖。
        assert!(
            save_settings_blocking_at(&path, &[("theme", Value::String("Nord".into()))]).is_err()
        );
        assert!(directory.path().join("settings.json").is_dir());
    }

    // ---- 零迁移断言(D06-2) ----

    #[test]
    fn legacy_portable_settings_are_neither_read_nor_written() {
        let (_data_dir, data_path) = fixture();
        let legacy_dir = tempfile::tempdir().unwrap();
        let legacy_path = legacy_dir.path().join("settings.json");
        let legacy = "{\n  \"appearance\": {\n    \"theme\": \"Nord\"\n  }\n}\n";
        write_json(&legacy_path, legacy);

        // 数据目录缺失时不得回退读取旧 exe 同级便携文件。
        let loaded = load_settings_at(&data_path).unwrap();
        assert!(!loaded.corrupted);
        assert_eq!(
            RuntimeSettings::from_raw(&loaded.data).theme,
            ThemeName::Linear,
            "旧便携文件不得影响新版读取"
        );

        // 新版写入只落数据目录,旧文件字节不变。
        save_settings_blocking_at(&data_path, &[("font_size", Value::Number(16.0))]).unwrap();
        assert_eq!(std::fs::read_to_string(&legacy_path).unwrap(), legacy);
        assert_eq!(
            read_tree(&data_path)
                .get("appearance")
                .unwrap()
                .get("font_size")
                .unwrap()
                .as_f64(),
            Some(16.0)
        );

        // 旧文件损坏与否也不改变新版读写行为。
        write_json(&legacy_path, "{ corrupt legacy");
        let reloaded = load_settings_at(&data_path).unwrap();
        assert!(!reloaded.corrupted);
        assert_eq!(
            RuntimeSettings::from_raw(&reloaded.data).theme,
            ThemeName::Linear
        );
        assert!(!legacy_dir.path().join("settings.corrupted.json").exists());
    }
}
