//! AgentCatalog 统一目录：每个 CLI 的内置命令、历史适配器与 Hooks 适配器标识。
//!
//! slug、显示名与别名仍由 [`AgentKind`] 提供；本模块只声明跨域消费的目录事实，
//! 让 UI、恢复流程与 Runtime 共用同一张表而不是各自维护 CLI 清单。

use crate::identity::AgentKind;

/// 原生历史适配器标识：只有声明适配器的 CLI 才允许展示扫描记录。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryAdapter {
    Claude,
    Codex,
}

/// Hooks 适配器标识：对应已声明的 CLI 注入适配（见 `design/hooks.md`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HooksAdapter {
    Claude,
    Codex,
    OpenCode,
    Cursor,
    Kimi,
    Pi,
    OhMyPi,
    Copilot,
    Grok,
}

/// 目录条目：只承载 [`AgentKind`] 之外的跨域事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentCatalogEntry {
    pub kind: AgentKind,
    /// 启动 CLI 的官方入口名；必须属于该 kind 已声明的别名。
    pub builtin_command: &'static str,
    pub history_adapter: Option<HistoryAdapter>,
    pub hooks_adapter: Option<HooksAdapter>,
    /// 内置 AgentRoot 相对 home 的路径；沙箱放行集与设置页目录表共用此值。
    pub default_root: Option<&'static str>,
}

impl AgentKind {
    /// 目录条目：全表显式声明，slug 改动不会静默改变启动命令、适配能力或默认根。
    pub fn catalog(self) -> AgentCatalogEntry {
        let (builtin_command, history_adapter, hooks_adapter, default_root) = match self {
            Self::Claude => (
                "claude",
                Some(HistoryAdapter::Claude),
                Some(HooksAdapter::Claude),
                Some(".claude"),
            ),
            Self::Codex => (
                "codex",
                Some(HistoryAdapter::Codex),
                Some(HooksAdapter::Codex),
                Some(".codex"),
            ),
            Self::Gemini => ("gemini", None, None, Some(".gemini")),
            Self::Aider => ("aider", None, None, Some(".aider")),
            Self::Amp => ("amp", None, None, Some(".config/amp")),
            Self::OpenCode => (
                "opencode",
                None,
                Some(HooksAdapter::OpenCode),
                Some(".config/opencode"),
            ),
            Self::Copilot => (
                "copilot",
                None,
                Some(HooksAdapter::Copilot),
                Some(".copilot"),
            ),
            Self::Cursor => ("agent", None, Some(HooksAdapter::Cursor), Some(".cursor")),
            Self::Goose => (
                "goose",
                None,
                None,
                Some("AppData/Roaming/Block/goose/config"),
            ),
            Self::Droid => ("droid", None, None, Some(".factory")),
            Self::Pi => ("pi", None, Some(HooksAdapter::Pi), Some(".pi/agent")),
            Self::Auggie => ("auggie", None, None, Some(".augment")),
            Self::Hermes => ("hermes", None, None, Some(".hermes")),
            Self::Vibe => ("vibe", None, None, Some(".vibe")),
            Self::Antigravity => ("agy", None, None, Some(".gemini/antigravity-cli")),
            Self::Grok => ("grok", None, Some(HooksAdapter::Grok), Some(".grok")),
            Self::Qwen => ("qwen", None, None, Some(".qwen")),
            Self::OhMyPi => ("omp", None, Some(HooksAdapter::OhMyPi), Some(".omp")),
            Self::Cline => ("cline", None, None, Some(".cline")),
            Self::Devin => ("devin", None, None, Some("AppData/Roaming/devin")),
            Self::Kimi => ("kimi", None, Some(HooksAdapter::Kimi), Some(".kimi-code")),
            Self::Kiro => ("kiro", None, None, Some(".kiro")),
            Self::Kilo => ("kilo", None, None, Some(".config/kilo")),
            Self::Qoder => ("qodercli", None, None, Some(".qoder")),
            Self::Maki => ("maki", None, None, Some("AppData/Roaming/maki")),
            Self::Trae => ("trae-cli", None, None, Some(".trae")),
            Self::CodeBuddy => ("codebuddy", None, None, Some(".codebuddy")),
        };
        AgentCatalogEntry {
            kind: self,
            builtin_command,
            history_adapter,
            hooks_adapter,
            default_root,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentKind, HistoryAdapter, HooksAdapter};

    /// 每个 CLI 恰好一个目录条目，slug 在全表唯一。
    #[test]
    fn catalog_covers_every_agent_kind() {
        let mut slugs = std::collections::HashSet::new();
        for kind in AgentKind::ALL {
            let entry = kind.catalog();
            assert_eq!(entry.kind, kind);
            assert!(
                slugs.insert(entry.kind.slug()),
                "slug 必须唯一：{}",
                entry.kind.slug()
            );
        }
        assert_eq!(slugs.len(), AgentKind::ALL.len());
    }

    /// 内置命令与别名共享 Windows 大小写规则的唯一命名空间，且别名形状合法。
    #[test]
    fn catalog_alias_namespace_is_unique_and_valid() {
        let mut owners = std::collections::HashMap::new();
        for kind in AgentKind::ALL {
            for name in std::iter::once(kind.slug()).chain(kind.aliases().iter().copied()) {
                assert!(!name.is_empty(), "{kind:?} 存在空名称");
                assert!(name.len() <= 32, "{name} 超过 32 字符");
                assert!(!name.starts_with('-'), "{name} 不得以 - 开头");
                assert!(
                    !name
                        .chars()
                        .any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '`')),
                    "{name} 含空白、引号或反引号"
                );
                if let Some(previous) = owners.insert(name.to_ascii_lowercase(), kind) {
                    assert_eq!(
                        previous, kind,
                        "别名 {name} 跨 CLI 冲突：{previous:?} 与 {kind:?}"
                    );
                }
            }
        }
    }

    /// 启动命令必须是该 CLI 已声明的官方别名，不引入第二套可执行名。
    #[test]
    fn catalog_builtin_command_is_declared_alias() {
        for kind in AgentKind::ALL {
            let entry = kind.catalog();
            assert!(
                kind.aliases().contains(&entry.builtin_command),
                "{kind:?} 的内置命令不属于官方别名：{}",
                entry.builtin_command
            );
        }
    }

    /// 内置 AgentRoot 相对 home 的路径是安全边界（沙箱放行集），逐项钉死。
    #[test]
    fn catalog_default_roots_are_pinned() {
        let expected = [
            ("claude", ".claude"),
            ("codex", ".codex"),
            ("gemini", ".gemini"),
            ("aider", ".aider"),
            ("amp", ".config/amp"),
            ("opencode", ".config/opencode"),
            ("copilot", ".copilot"),
            ("cursor", ".cursor"),
            ("goose", "AppData/Roaming/Block/goose/config"),
            ("droid", ".factory"),
            ("pi", ".pi/agent"),
            ("auggie", ".augment"),
            ("hermes", ".hermes"),
            ("vibe", ".vibe"),
            ("antigravity", ".gemini/antigravity-cli"),
            ("grok", ".grok"),
            ("qwen", ".qwen"),
            ("omp", ".omp"),
            ("cline", ".cline"),
            ("devin", "AppData/Roaming/devin"),
            ("kimi", ".kimi-code"),
            ("kiro", ".kiro"),
            ("kilo", ".config/kilo"),
            ("qodercli", ".qoder"),
            ("maki", "AppData/Roaming/maki"),
            ("trae-cli", ".trae"),
            ("codebuddy", ".codebuddy"),
        ];
        let actual = AgentKind::ALL
            .into_iter()
            .map(|kind| (kind.slug(), kind.catalog().default_root))
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len());
        for ((slug, root), (expected_slug, expected_root)) in actual.iter().zip(expected) {
            assert_eq!(*slug, expected_slug);
            assert_eq!(*root, Some(expected_root), "{slug} 默认根被意外改动");
        }
    }

    /// 历史适配器只接入 Claude／Codex；Hooks 适配器与 slterm_hook 能力集一致。
    #[test]
    fn catalog_declares_history_and_hooks_adapters() {
        for kind in AgentKind::ALL {
            let entry = kind.catalog();
            let has_history = matches!(kind, AgentKind::Claude | AgentKind::Codex);
            assert_eq!(
                entry.history_adapter.is_some(),
                has_history,
                "{kind:?} 历史适配器声明与接入清单不符"
            );
            if has_history {
                assert!(matches!(
                    entry.history_adapter,
                    Some(HistoryAdapter::Claude | HistoryAdapter::Codex)
                ));
            }
            let has_hooks = matches!(
                kind,
                AgentKind::Claude
                    | AgentKind::Codex
                    | AgentKind::OpenCode
                    | AgentKind::Cursor
                    | AgentKind::Kimi
                    | AgentKind::Pi
                    | AgentKind::OhMyPi
                    | AgentKind::Copilot
                    | AgentKind::Grok
            );
            assert_eq!(
                entry.hooks_adapter.is_some(),
                has_hooks,
                "{kind:?} Hooks 适配器声明与注入适配清单不符"
            );
            if has_hooks {
                assert!(matches!(
                    entry.hooks_adapter,
                    Some(
                        HooksAdapter::Claude
                            | HooksAdapter::Codex
                            | HooksAdapter::OpenCode
                            | HooksAdapter::Cursor
                            | HooksAdapter::Kimi
                            | HooksAdapter::Pi
                            | HooksAdapter::OhMyPi
                            | HooksAdapter::Copilot
                            | HooksAdapter::Grok
                    )
                ));
            }
        }
    }
}
