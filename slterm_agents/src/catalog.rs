//! AgentCatalog 统一目录：每个 CLI 的内置命令、历史适配器与 Hooks 适配器标识。
//!
//! slug、显示名与别名仍由 [`AgentKind`] 提供；本模块只声明跨域消费的目录事实，
//! 让 UI、恢复流程与 Runtime 共用同一张表而不是各自维护 CLI 清单。

use crate::identity::AgentKind;
use slterm_settings::AgentRootId;

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

impl AgentKind {
    /// 内置 AgentRoot 的稳定身份；固定 UUIDv7 字节表，跨重启不变。
    pub fn builtin_root_id(self) -> AgentRootId {
        let bytes = match self {
            Self::Claude => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x00, 0x70, 0x00, 0x80, 0x3c, 0x0f, 0x1e, 0x69, 0x78,
                0x4b, 0x5a,
            ],
            Self::Codex => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x01, 0x70, 0x07, 0x81, 0x1d, 0x6c, 0x7f, 0x4e, 0x59,
                0xa8, 0xbb,
            ],
            Self::Gemini => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x02, 0x70, 0x0e, 0x82, 0x72, 0x4d, 0x5c, 0xaf, 0xbe,
                0x89, 0x98,
            ],
            Self::Aider => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x03, 0x70, 0x15, 0x83, 0x53, 0xa2, 0xbd, 0x8c, 0x9f,
                0xee, 0xf9,
            ],
            Self::Amp => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x04, 0x70, 0x1c, 0x84, 0xb0, 0x83, 0x92, 0xed, 0xfc,
                0xcf, 0xde,
            ],
            Self::OpenCode => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x05, 0x70, 0x23, 0x85, 0x91, 0xe0, 0xf3, 0xc2, 0xdd,
                0x2c, 0x3f,
            ],
            Self::Copilot => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x06, 0x70, 0x2a, 0x86, 0xf6, 0xc1, 0xd0, 0x23, 0x32,
                0x0d, 0x1c,
            ],
            Self::Cursor => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x07, 0x70, 0x31, 0x87, 0xd7, 0x26, 0x31, 0x00, 0x13,
                0x62, 0x7d,
            ],
            Self::Goose => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x08, 0x70, 0x38, 0x88, 0x34, 0x07, 0x16, 0x61, 0x70,
                0x43, 0x52,
            ],
            Self::Droid => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x09, 0x70, 0x3f, 0x89, 0x15, 0x64, 0x77, 0x46, 0x51,
                0xa0, 0xb3,
            ],
            Self::Pi => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x0a, 0x70, 0x46, 0x8a, 0x6a, 0x45, 0x54, 0xa7, 0xb6,
                0x81, 0x90,
            ],
            Self::Auggie => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x0b, 0x70, 0x4d, 0x8b, 0x4b, 0x5a, 0xb5, 0x84, 0x97,
                0xe6, 0xf1,
            ],
            Self::Hermes => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x0c, 0x70, 0x54, 0x8c, 0xa8, 0xbb, 0x8a, 0xe5, 0xf4,
                0xc7, 0xd6,
            ],
            Self::Vibe => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x0d, 0x70, 0x5b, 0x8d, 0x89, 0x98, 0xeb, 0xfa, 0xd5,
                0x24, 0x37,
            ],
            Self::Antigravity => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x0e, 0x70, 0x62, 0x8e, 0xee, 0xf9, 0xc8, 0xdb, 0x2a,
                0x05, 0x14,
            ],
            Self::Grok => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x0f, 0x70, 0x69, 0x8f, 0xcf, 0xde, 0x29, 0x38, 0x0b,
                0x1a, 0x75,
            ],
            Self::Qwen => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x10, 0x70, 0x70, 0x90, 0x2c, 0x3f, 0x0e, 0x19, 0x68,
                0x7b, 0x4a,
            ],
            Self::OhMyPi => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x11, 0x70, 0x77, 0x91, 0x0d, 0x1c, 0x6f, 0x7e, 0x49,
                0x58, 0xab,
            ],
            Self::Cline => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x12, 0x70, 0x7e, 0x92, 0x62, 0x7d, 0x4c, 0x5f, 0xae,
                0xb9, 0x88,
            ],
            Self::Devin => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x13, 0x70, 0x85, 0x93, 0x43, 0x52, 0xad, 0xbc, 0x8f,
                0x9e, 0xe9,
            ],
            Self::Kimi => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x14, 0x70, 0x8c, 0x94, 0xa0, 0xb3, 0x82, 0x9d, 0xec,
                0xff, 0xce,
            ],
            Self::Kiro => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x15, 0x70, 0x93, 0x95, 0x81, 0x90, 0xe3, 0xf2, 0xcd,
                0xdc, 0x2f,
            ],
            Self::Kilo => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x16, 0x70, 0x9a, 0x96, 0xe6, 0xf1, 0xc0, 0xd3, 0x22,
                0x3d, 0x0c,
            ],
            Self::Qoder => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x17, 0x70, 0xa1, 0x97, 0xc7, 0xd6, 0x21, 0x30, 0x03,
                0x12, 0x6d,
            ],
            Self::Maki => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x18, 0x70, 0xa8, 0x98, 0x24, 0x37, 0x06, 0x11, 0x60,
                0x73, 0x42,
            ],
            Self::Trae => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x19, 0x70, 0xaf, 0x99, 0x05, 0x14, 0x67, 0x76, 0x41,
                0x50, 0xa3,
            ],
            Self::CodeBuddy => [
                0x01, 0x9b, 0x76, 0xda, 0xa8, 0x1a, 0x70, 0xb6, 0x9a, 0x1a, 0x75, 0x44, 0x57, 0xa6,
                0xb1, 0x80,
            ],
        };
        AgentRootId::from_bytes(bytes).expect("内置根身份必须是合法 UUIDv7")
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

    /// 内置 AgentRoot 身份全表唯一且为合法 UUIDv7；跨重启稳定。
    #[test]
    fn catalog_builtin_root_ids_are_unique_v7() {
        let mut seen = std::collections::HashSet::new();
        for kind in AgentKind::ALL {
            let id = kind.builtin_root_id();
            assert_eq!(id.as_bytes()[6] >> 4, 7, "{kind:?} 内置根身份不是 UUIDv7");
            assert!(seen.insert(id), "{kind:?} 内置根身份重复");
        }
        assert_eq!(seen.len(), AgentKind::ALL.len());
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
