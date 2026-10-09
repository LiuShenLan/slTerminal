//! 每 CLI 的 hook 接入偏好段。缺键继承全局兜底开关,全部挂在
//! settings.json 的 `agent_hooks` 段内(段嵌套形态)。
//!
//! 安装权威(03 篇 ensure_*_hooks 族)读 `enabled` 做接入/卸载决策;
//! `all_updates`/`setup_updates` 是 CLI setup 与恢复默认路径的写盘入口,
//! JSON 化后值形态 = 真布尔(不再是 txt 时代的 "1"/"0" 字符串)。

use crate::RawSettings;
use crate::json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentHook {
    Claude,
    Codex,
    OpenCode,
    Pi,
    Copilot,
    Grok,
    OhMyPi,
    Cursor,
    Kimi,
}

impl AgentHook {
    /// 默认仅两家开启。
    pub const DEFAULT: [Self; 2] = [Self::Claude, Self::Codex];

    pub const ALL: [Self; 9] = [
        Self::Claude,
        Self::Codex,
        Self::OpenCode,
        Self::Pi,
        Self::Copilot,
        Self::Grok,
        Self::OhMyPi,
        Self::Cursor,
        Self::Kimi,
    ];

    /// 全局兜底键(历史全局开关语义,挂 agent_hooks 段内)。
    pub const GLOBAL_SETTINGS_KEY: &'static str = "slterm_ai_hooks";

    pub const fn settings_key(self) -> &'static str {
        match self {
            Self::Claude => "slterm_ai_hooks_claude",
            Self::Codex => "slterm_ai_hooks_codex",
            Self::OpenCode => "slterm_ai_hooks_opencode",
            Self::Pi => "slterm_ai_hooks_pi",
            Self::Copilot => "slterm_ai_hooks_copilot",
            Self::Grok => "slterm_ai_hooks_grok",
            Self::OhMyPi => "slterm_ai_hooks_omp",
            Self::Cursor => "slterm_ai_hooks_cursor",
            Self::Kimi => "slterm_ai_hooks_kimi",
        }
    }

    /// 每 agent 独立键 → 全局键兜底 → 默认两家。
    pub fn enabled(self, raw: &RawSettings) -> bool {
        let legacy_default = Self::DEFAULT.contains(&self);
        raw.bool_on(self.settings_key()).unwrap_or_else(|| {
            legacy_default && raw.bool_on(Self::GLOBAL_SETTINGS_KEY).unwrap_or(true)
        })
    }

    /// 显式 CLI 安装/卸载写全部(含 UI 里的逐项覆盖)。
    pub fn all_updates(enabled: bool) -> Vec<(&'static str, Value)> {
        std::iter::once((Self::GLOBAL_SETTINGS_KEY, Value::Bool(enabled)))
            .chain(Self::ALL.map(|agent| (agent.settings_key(), Value::Bool(enabled))))
            .collect()
    }

    /// 安装器/CLI 恢复默认的两个入口;其他 Agent 保留用户在菜单中的选择。
    pub fn setup_updates() -> Vec<(&'static str, Value)> {
        std::iter::once((Self::GLOBAL_SETTINGS_KEY, Value::Bool(true)))
            .chain(Self::DEFAULT.map(|agent| (agent.settings_key(), Value::Bool(true))))
            .collect()
    }
}

#[cfg(test)]
mod agent_hooks_tests {
    use super::*;
    use crate::{apply_updates, json};

    fn raw(text: &str) -> RawSettings {
        RawSettings::from_json_bytes(text.as_bytes())
    }

    #[test]
    fn individual_keys_override_and_fall_back_to_the_global_switch() {
        for agent in AgentHook::ALL {
            assert_eq!(
                agent.enabled(&RawSettings::default()),
                AgentHook::DEFAULT.contains(&agent)
            );
            assert!(!agent.enabled(&raw(r#"{"agent_hooks": {"slterm_ai_hooks": false}}"#)));
        }
        // 独立键优先于全局键;类型不符的键视为未设置。
        let settings = raw(r#"{"agent_hooks": {
                "slterm_ai_hooks": false,
                "slterm_ai_hooks_codex": true,
                "slterm_ai_hooks_kimi": "invalid"
            }}"#);
        assert!(AgentHook::Codex.enabled(&settings));
        assert!(!AgentHook::Claude.enabled(&settings));
        assert!(!AgentHook::Kimi.enabled(&settings), "类型不符回全局兜底");
    }

    #[test]
    fn all_updates_write_every_agent_and_preserve_unrelated_data() {
        let original = json::parse(
            br#"{
  "appearance": {
    "theme": "Linear"
  },
  "user_notes": {
    "handmade": "keep"
  }
}
"#,
        )
        .unwrap();
        let merged = apply_updates(&original, &AgentHook::all_updates(false));
        let settings = RawSettings::from_json_bytes(json::to_string_pretty(&merged).as_bytes());
        for agent in AgentHook::ALL {
            assert!(!agent.enabled(&settings));
        }
        // 未知段与其他段原样保留。
        assert_eq!(
            merged
                .get("user_notes")
                .unwrap()
                .get("handmade")
                .unwrap()
                .as_str(),
            Some("keep")
        );
        assert_eq!(
            merged
                .get("appearance")
                .unwrap()
                .get("theme")
                .unwrap()
                .as_str(),
            Some("Linear")
        );
        let enabled = apply_updates(&merged, &AgentHook::all_updates(true));
        let settings = RawSettings::from_json_bytes(json::to_string_pretty(&enabled).as_bytes());
        for agent in AgentHook::ALL {
            assert!(agent.enabled(&settings));
        }
    }

    #[test]
    fn setup_updates_enable_only_defaults_and_preserve_optional_choices() {
        let original = json::parse(
            br#"{"agent_hooks": {
                "slterm_ai_hooks": false,
                "slterm_ai_hooks_grok": true,
                "slterm_ai_hooks_pi": false
            }}"#,
        )
        .unwrap();
        let merged = apply_updates(&original, &AgentHook::setup_updates());
        let settings = RawSettings::from_json_bytes(json::to_string_pretty(&merged).as_bytes());
        assert!(AgentHook::Claude.enabled(&settings) && AgentHook::Codex.enabled(&settings));
        assert!(
            AgentHook::Grok.enabled(&settings),
            "显式选择不被恢复默认覆盖"
        );
        assert!(!AgentHook::Pi.enabled(&settings) && !AgentHook::Kimi.enabled(&settings));
    }
}
