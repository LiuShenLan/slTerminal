//! 共享安装权威:平台适配器(local/*)供文件与命令,归属、provider 事件表
//! 与 feature 兼容判定只有这一处。
//!
//! `CodexHookMode` 本体与事件表(`events()`/`CODEX_*_EVENTS`)锚在
//! `event.rs`(协议解析与安装器共用同一张表);本模块只扩展安装器专用的
//! `argument()` 与配置生成/合并/feature 开关。

use serde_json::{Value, json};

use super::CodexHookMode;

mod notify;
pub(crate) use notify::{desired_codex_notify, has_codex_notify, stripped_codex_notify};

/// 0.154.0 是本机实测验证过的完整合同,不是猜的历史引入版本。更老但广告了
/// hooks 的客户端用三事件合同;没有该 feature 的客户端只保留 notify。
pub(crate) fn codex_mode(version: &str, features: &str) -> Option<CodexHookMode> {
    if !features
        .lines()
        .any(|line| line.split_whitespace().next() == Some("hooks"))
    {
        return None;
    }
    let version = version.trim().strip_prefix("codex-cli ")?;
    let mut parts = version.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts.next()?.split('-').next()?.parse().ok()?;
    Some(if (major, minor, patch) >= (0, 154, 0) {
        CodexHookMode::Full
    } else {
        CodexHookMode::Turns
    })
}

impl CodexHookMode {
    /// 写进 codex hooks.json 的命令行尾巴(helper argv 形态,与 slterm_hook
    /// 的 `--hooks=turns|full` 入口互钉)。
    pub(crate) fn argument(self) -> &'static str {
        match self {
            Self::Turns => "--hooks=turns",
            Self::Full => "--hooks=full",
        }
    }
}

/// 用 provider 自己的命令 shell 形态生成 hook 组。任何 hook 都不得在投递
/// 失败时输出权限决定、stdout 上下文或非零退出码。
pub(crate) fn codex_groups(
    command: &str,
    windows_command: Option<&str>,
    mode: CodexHookMode,
) -> Value {
    let mut groups = serde_json::Map::new();
    for &event in mode.events() {
        let mut handler = json!({"type":"command", "command":command, "timeout":3});
        if let Some(windows) = windows_command {
            handler["commandWindows"] = json!(windows);
        }
        let mut group = json!({"hooks":[handler]});
        if event == "PreToolUse" {
            // 用户提问是独立的结构化工具,不是权限请求;普通工具绝不能变成
            // attention 事件。
            group["matcher"] = json!("^request_user_input$");
        }
        groups.insert(event.to_owned(), json!([group]));
    }
    Value::Object(groups)
}

/// marker 记录我们安装过的精确组。绝不因为某条命令包含产品名就认领;
/// 被用户编辑过的组报「edited … hook preserved」错而不覆盖,卸载恢复
/// marker 之前的样子。
pub(crate) fn merge_groups(
    raw: Option<&str>,
    previous: Option<&str>,
    desired: &Value,
) -> Result<(String, String), String> {
    let mut root: Value = match raw {
        Some(raw) => serde_json::from_str(raw).map_err(|error| error.to_string())?,
        None => json!({}),
    };
    let previous: Value = match previous {
        Some(raw) => serde_json::from_str(raw).map_err(|error| error.to_string())?,
        None => json!({}),
    };
    let object = root
        .as_object_mut()
        .ok_or("hook config must be an object")?;
    let hooks = object.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks.as_object_mut().ok_or("hooks must be an object")?;
    let previous = previous
        .as_object()
        .ok_or("hook ownership marker must be an object")?;
    let desired = desired
        .as_object()
        .ok_or("desired hooks must be an object")?;
    for (event, owned) in previous {
        let owned = owned.as_array().ok_or("owned hook groups must be arrays")?;
        let Some(current) = hooks.get_mut(event) else {
            continue;
        };
        let current = current.as_array_mut().ok_or("hook groups must be arrays")?;
        for group in owned {
            if current.iter().any(|item| item == group) {
                current.retain(|item| item != group);
            } else if !current.is_empty() {
                return Err(format!("edited {event} hook preserved"));
            }
        }
    }
    hooks.retain(|_, value| !value.as_array().is_some_and(Vec::is_empty));
    for (event, groups) in desired {
        let groups = groups
            .as_array()
            .ok_or("desired hook groups must be arrays")?;
        let current = hooks.entry(event).or_insert_with(|| json!([]));
        let current = current.as_array_mut().ok_or("hook groups must be arrays")?;
        for group in groups {
            if !current.contains(group) {
                current.push(group.clone());
            }
        }
    }
    Ok((
        serde_json::to_string_pretty(&root).map_err(|error| error.to_string())? + "\n",
        serde_json::to_string_pretty(&Value::Object(desired.clone()))
            .map_err(|error| error.to_string())?
            + "\n",
    ))
}

/// 保注释、保一切无关 TOML。显式 opt-out(hooks=false)优先于自动安装。
/// 返回是否本次引入了 features 键,卸载据此精准还原而不误关别家集成。
pub(crate) fn enable_codex_feature(raw: &str) -> Result<Option<(String, bool)>, String> {
    let mut doc = raw
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| error.to_string())?;
    for key in ["hooks", "codex_hooks"] {
        if doc
            .get("features")
            .and_then(|item| item.get(key))
            .and_then(|item| item.as_bool())
            == Some(false)
        {
            return Ok(None);
        }
    }
    let current = doc.get("features").and_then(|item| item.get("hooks"));
    if current.is_some_and(|item| item.as_bool().is_none()) {
        return Err("features.hooks must be a boolean".into());
    }
    let introduced = current.is_none();
    if introduced {
        if doc.get("features").is_none() {
            doc["features"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        if !doc["features"].is_table_like() {
            return Err("features must be a table".into());
        }
        doc["features"]["hooks"] = toml_edit::value(true);
    }
    Ok(Some((doc.to_string(), introduced)))
}

#[cfg(test)]
mod installation_tests {
    use super::*;

    #[test]
    fn version_and_advertised_feature_select_the_installed_contract() {
        assert_eq!(
            codex_mode("codex-cli 0.154.0", "hooks stable true"),
            Some(CodexHookMode::Full)
        );
        assert_eq!(
            codex_mode("codex-cli 0.120.0", "hooks experimental false"),
            Some(CodexHookMode::Turns)
        );
        assert_eq!(codex_mode("codex-cli 0.154.0", "other stable true"), None);
        assert_eq!(codex_mode("invalid", "hooks stable true"), None);
    }

    #[test]
    fn merge_upgrade_and_remove_preserve_foreign_hooks_and_metadata() {
        let foreign =
            json!({"hooks":[{"type":"command","command":"user-tool"}],"matcher":"custom"});
        let original = json!({"description":"user","hooks":{"Stop":[foreign]}}).to_string();
        let desired = codex_groups("old-helper", None, CodexHookMode::Turns);
        let (first, marker) = merge_groups(Some(&original), None, &desired).unwrap();
        assert_eq!(
            merge_groups(Some(&first), Some(&marker), &desired).unwrap(),
            (first.clone(), marker.clone())
        );
        let full = codex_groups("new-helper", Some("windows-helper"), CodexHookMode::Full);
        let (upgraded, marker) = merge_groups(Some(&first), Some(&marker), &full).unwrap();
        let root: Value = serde_json::from_str(&upgraded).unwrap();
        assert_eq!(root["hooks"]["Stop"].as_array().unwrap().len(), 2);
        let (removed, _) = merge_groups(Some(&upgraded), Some(&marker), &json!({})).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&removed).unwrap(),
            serde_json::from_str::<Value>(&original).unwrap()
        );
    }

    #[test]
    fn edited_and_malformed_hook_configs_are_not_overwritten() {
        let desired = codex_groups("helper", None, CodexHookMode::Full);
        let (raw, marker) = merge_groups(None, None, &desired).unwrap();
        let mut edited: Value = serde_json::from_str(&raw).unwrap();
        edited["hooks"]["Stop"][0]["hooks"][0]["timeout"] = json!(9);
        assert!(merge_groups(Some(&edited.to_string()), Some(&marker), &desired).is_err());
        for bad in [
            "[]",
            "{broken",
            r#"{"hooks":[]}"#,
            r#"{"hooks":{"Stop":true}}"#,
        ] {
            assert!(merge_groups(Some(bad), None, &desired).is_err());
        }
    }

    #[test]
    fn feature_installation_respects_user_opt_out_and_comments() {
        assert!(
            enable_codex_feature("[features]\nhooks = false\n")
                .unwrap()
                .is_none()
        );
        assert!(
            enable_codex_feature("[features]\ncodex_hooks = false\n")
                .unwrap()
                .is_none()
        );
        let (result, introduced) = enable_codex_feature("# keep\nmodel = 'user-model'\n")
            .unwrap()
            .unwrap();
        assert!(introduced && result.contains("# keep") && result.contains("'user-model'"));
        assert!(!enable_codex_feature(&result).unwrap().unwrap().1);
        assert!(enable_codex_feature("features = 3\n").is_err());
    }
}
