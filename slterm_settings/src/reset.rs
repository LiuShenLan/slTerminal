//! 恢复默认设置的纯值投影：从原始值树删除注册表 resettable 键。
//! 未知键、用户手改段与授权类选择(agent_hooks 段)原样保留；备份、原子写与
//! 审计归 application 的设置写通道。
//!
//! 覆盖对账由 `keys` 注册表测试机械保证:from_raw 消费键集与 resettable
//! 集合全等,新增键忘了登记即红。

use crate::json::Value;
use crate::keys;

/// 纯函数:删除全部 resettable 已知键(段内移除),未知段/未知键/
/// agent_hooks 授权选择原样保留。段骨架保留(空段是「用户曾有过设置」
/// 的无害痕迹)。
pub fn default_settings_tree(tree: &Value) -> Value {
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

#[cfg(test)]
mod reset_tests {
    use super::*;
    use crate::json;
    use crate::{AgentHook, RawSettings, RuntimeSettings};

    fn raw(value: &Value) -> RawSettings {
        RawSettings::from_json_bytes(json::to_string_pretty(value).as_bytes())
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
    "font_size": 18
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
        assert_eq!(settings.font_size_px, None);
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
}
