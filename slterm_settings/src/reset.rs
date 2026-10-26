//! 恢复默认设置(JSON 化形态):备份 → 删除注册表 resettable 键 → 原子写。
//! 未知键、用户手改段与授权类选择(agent_hooks 段)原样保留。
//!
//! 覆盖对账由 `keys` 注册表测试机械保证:from_raw 消费键集与 resettable
//! 集合全等,新增键忘了登记即红。

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::json::{self, Value};
use crate::keys;

static RESET_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 纯函数:删除全部 resettable 已知键(段内移除),未知段/未知键/
/// agent_hooks 授权选择原样保留。段骨架保留(空段是「用户曾有过设置」
/// 的无害痕迹)。
fn default_settings_tree(tree: &Value) -> Value {
    let mut restored = tree.clone();
    for def in keys::KEYS.iter().filter(|def| def.resettable) {
        if let Some(section) = restored
            .object_mut()
            .and_then(|members| {
                members
                    .iter_mut()
                    .find(|(name, _)| name == def.domain.segment())
                    .map(|(_, value)| value)
            })
            .and_then(|section| section.object_mut())
        {
            section.retain(|(name, _)| name != def.key);
        }
    }
    restored
}

/// 显式路径入口；调用方从启动时解析的 `AppPaths` 提供设置文件位置。
pub fn restore_default_settings_at(path: &Path) -> io::Result<Option<PathBuf>> {
    // 损坏文件直接报错,不覆盖(读侧宽容、写侧拒绝的刻意不对称)。
    let original = match fs::read(path) {
        Ok(bytes) => Some(
            json::parse(&bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    // 无文件 = 无可重置,不动盘。
    let Some(tree) = original else {
        return Ok(None);
    };
    let restored = default_settings_tree(&tree);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = RESET_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let suffix = format!("{timestamp}-{}-{sequence}", std::process::id());
    let backup = path.with_extension(format!("before-reset-{suffix}.bak"));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)?;
        file.write_all(json::to_string_pretty(&tree).as_bytes())?;
        file.sync_all()?;
    }
    let temporary = path.with_extension(format!("reset-{suffix}.tmp"));
    let written = {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = file
            .write_all(json::to_string_pretty(&restored).as_bytes())
            .and_then(|_| file.sync_all());
        drop(file);
        result
    };
    let result = written.and_then(|_| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map(|_| Some(backup))
}

#[cfg(test)]
mod reset_tests {
    use super::*;
    use crate::{AgentHook, RawSettings, RuntimeSettings};

    fn raw(value: &Value) -> RawSettings {
        RawSettings::from_json_bytes(json::to_string_pretty(value).as_bytes())
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "slterm-reset-{tag}-{}-{}",
            std::process::id(),
            RESET_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn reset_removes_known_keys_and_keeps_unknown_and_authorized_data() {
        let tree = json::parse(
            br##"{
  "appearance": {
    "theme": "Nord",
    "opacity": 0.65,
    "background": "#101216",
    "custom_theme": "my-night",
    "pane_card_radius": 20
  },
  "scrolling": {
    "scrollback_lines": 100000,
    "scroll_speed": 4.0
  },
  "agent_hooks": {
    "slterm_ai_hooks": false,
    "slterm_ai_hooks_grok": true
  },
  "handmade_section": {
    "note": "keep"
  },
  "keybind": [
    { "combo": "ctrl+x", "action": "cut" }
  ]
}
"##,
        )
        .unwrap();
        let restored = default_settings_tree(&tree);
        let settings = RuntimeSettings::from_raw(&raw(&restored));
        let defaults = RuntimeSettings::from_raw(&RawSettings::default());
        assert_eq!(settings.theme, defaults.theme);
        assert_eq!(settings.opacity, 1.0);
        assert!(!settings.opacity_is_explicit());
        assert_eq!(settings.scrollback_lines, 10_000);
        assert_eq!(settings.scroll_speed, 1.0);
        assert_eq!(settings.custom_theme, None);
        assert_eq!(settings.pane_card_radius, None);
        // 授权类选择原样保留(resettable=false)。
        let hooks = restored.get("agent_hooks").unwrap();
        assert_eq!(hooks.get("slterm_ai_hooks").unwrap().as_bool(), Some(false));
        assert_eq!(
            hooks.get("slterm_ai_hooks_grok").unwrap().as_bool(),
            Some(true)
        );
        // 未知段与域外段原样保留。
        assert_eq!(
            restored
                .get("handmade_section")
                .unwrap()
                .get("note")
                .unwrap()
                .as_str(),
            Some("keep")
        );
        assert!(matches!(restored.get("keybind"), Some(Value::Array(_))));
    }

    /// 三清单对账(RESET_KEYS 侧):对「全键存在」的文档执行重置后,
    /// 消失的键集合必须精确等于 `keys::reset_keys()`——resettable=false
    /// 的授权类选择与其他段原样保留,无孤儿键、无漏删。
    #[test]
    fn reset_removal_set_matches_the_registry_reset_list() {
        let mut tree = Value::empty_object();
        for def in keys::KEYS {
            let segment = def.domain.segment();
            if !matches!(tree.get(segment), Some(Value::Object(_))) {
                tree.set(segment, Value::empty_object());
            }
            tree.object_mut()
                .unwrap()
                .iter_mut()
                .find(|(name, _)| name == segment)
                .unwrap()
                .1
                .set(def.key, Value::Bool(true));
        }
        tree.set("handmade", Value::empty_object());
        let restored = default_settings_tree(&tree);
        for def in keys::KEYS {
            let present = restored
                .get(def.domain.segment())
                .and_then(|section| section.get(def.key))
                .is_some();
            assert_eq!(
                present, !def.resettable,
                "键 {} 的重置归属与注册表不一致",
                def.key
            );
        }
        assert!(restored.get("handmade").is_some(), "未知段不参与重置删除");
        let mut actually_removed: Vec<_> = keys::KEYS
            .iter()
            .filter(|def| {
                restored
                    .get(def.domain.segment())
                    .and_then(|section| section.get(def.key))
                    .is_none()
            })
            .map(|def| def.key)
            .collect();
        let mut listed: Vec<_> = keys::reset_keys().collect();
        actually_removed.sort_unstable();
        listed.sort_unstable();
        assert_eq!(
            actually_removed, listed,
            "重置实际删除集合必须精确等于 RESET_KEYS"
        );
    }

    #[test]
    fn reset_preserves_every_hooks_authorization_choice() {
        let tree = json::parse(
            br#"{"agent_hooks": {
                "slterm_ai_hooks": false,
                "slterm_ai_hooks_claude": false,
                "slterm_ai_hooks_codex": true,
                "slterm_ai_hooks_grok": true
            }}"#,
        )
        .unwrap();
        let before = raw(&tree);
        let after = raw(&default_settings_tree(&tree));
        for agent in AgentHook::ALL {
            assert_eq!(agent.enabled(&before), agent.enabled(&after), "{agent:?}");
        }
    }

    #[test]
    fn reset_backs_up_the_original_without_touching_adjacent_files() {
        let directory = temp_dir("backup");
        let path = directory.join("settings.json");
        let sibling = directory.join("notes.json");
        fs::write(
            &path,
            "{\n  \"appearance\": {\n    \"theme\": \"Nord\"\n  }\n}\n",
        )
        .unwrap();
        fs::write(&sibling, "{\"user\": \"data\"}").unwrap();

        let backup = restore_default_settings_at(&path)
            .unwrap()
            .expect("backup produced");
        // 备份含原文;现文件已无已知键。
        let backup_tree = json::parse(&fs::read(&backup).unwrap()).unwrap();
        assert_eq!(
            backup_tree
                .get("appearance")
                .unwrap()
                .get("theme")
                .unwrap()
                .as_str(),
            Some("Nord")
        );
        let current = json::parse(&fs::read(&path).unwrap()).unwrap();
        assert!(current.get("appearance").unwrap().get("theme").is_none());
        assert_eq!(
            fs::read_to_string(&sibling).unwrap(),
            "{\"user\": \"data\"}"
        );

        // 再次重置产生新备份,不覆盖旧备份。
        let next = restore_default_settings_at(&path)
            .unwrap()
            .expect("second backup");
        assert_ne!(backup, next);
        assert!(backup.exists() && next.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_settings_file_is_not_overwritten_and_missing_file_is_a_noop() {
        let directory = temp_dir("invalid");
        let path = directory.join("settings.json");
        // 无文件:不动盘,无备份。
        assert_eq!(restore_default_settings_at(&path).unwrap(), None);
        assert!(!path.exists());
        // 损坏文件:报错且不覆盖。
        fs::write(&path, "{ not json").unwrap();
        assert!(restore_default_settings_at(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
        fs::remove_dir_all(directory).unwrap();
    }
}
