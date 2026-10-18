//! 设置页 hooks 编辑域(轨二,06 篇双轨缝合)。
//!
//! 双轨不交叉的判据 = marker 归属:含 slterm-hook 命令的 matcher 组归
//! 安装器专属,编辑器只读、保存时从磁盘原样保留;编辑域限用户/第三方
//! handler。写入前跑 SEC-05 语义校验(事件名白名单 + command 型必填),
//! 校验失败零写盘;非法 JSON / 非对象根一律拒绝编辑,绝不「修复式覆盖」。
//!
//! 编辑面是 claude 用户层 `~/.claude/settings.json`(设置页无 project 上下文;
//! user/project/local 三层完整编辑面归 03 篇)。数据源与安装动作归
//! `integrations`/`local`,本模块只做文档级读-改-写。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::CLAUDE_EVENTS;

/// 一份 hooks 编辑文档:可编辑子树 + 安装器认领组只读快照。
///
/// `editable` / `owned` 都是「事件名 → matcher 组数组」对象,只在有组的
/// 事件上保留键。保存走 read-modify-write:磁盘上的认领组原样保留,非
/// 认领组整段替换为编辑结果。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HooksDocument {
    path: PathBuf,
    editable: Value,
    owned: Value,
}

impl HooksDocument {
    /// 生产路径:claude 用户层 settings.json。目录不存在(未装 Claude)时
    /// 拒绝进入编辑,不为用户 scaffold 配置树。
    pub(crate) fn load_user() -> Result<Self, String> {
        let dir = super::local::claude_config_dir()
            .ok_or_else(|| "无法解析 Claude 配置目录".to_owned())?;
        if !dir.is_dir() {
            return Err("Claude 配置目录不存在;先启动一次 Claude,然后刷新".to_owned());
        }
        Self::load_at(&dir.join("settings.json"))
    }

    /// 显式路径加载(设置页注入端与测试 tempdir 假 home)。
    pub(crate) fn load_at(path: &Path) -> Result<Self, String> {
        let root = read_root(path)?;
        let root = root
            .as_object()
            .ok_or_else(|| "配置文件根必须是 JSON 对象".to_owned())?;
        let hooks = match root.get("hooks") {
            None => serde_json::Map::new(),
            Some(hooks) => hooks
                .as_object()
                .cloned()
                .ok_or_else(|| "hooks 子树必须是 JSON 对象".to_owned())?,
        };
        let mut editable = serde_json::Map::new();
        let mut owned = serde_json::Map::new();
        for (event, groups) in hooks {
            let groups = groups
                .as_array()
                .ok_or_else(|| format!("事件 {event} 的值必须是数组"))?;
            let mut editable_groups = Vec::new();
            let mut owned_groups = Vec::new();
            for group in groups {
                if group_is_installer_owned(group)
                    .ok_or_else(|| format!("事件 {event} 的 matcher 组形态非法"))?
                {
                    owned_groups.push(group.clone());
                } else {
                    editable_groups.push(group.clone());
                }
            }
            if !editable_groups.is_empty() {
                editable.insert(event.clone(), Value::Array(editable_groups));
            }
            if !owned_groups.is_empty() {
                owned.insert(event, Value::Array(owned_groups));
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            editable: Value::Object(editable),
            owned: Value::Object(owned),
        })
    }

    /// 可编辑子树的 JSON 文本(编辑器显示值;事件名 → 用户 matcher 组)。
    pub(crate) fn editable_json(&self) -> String {
        serde_json::to_string_pretty(&self.editable).unwrap_or_else(|_| "{}".to_owned())
    }

    /// 安装器认领组数量(只读展示;双轨判据的可见面)。
    pub(crate) fn owned_group_count(&self) -> usize {
        self.owned
            .as_object()
            .map(|events| {
                events
                    .values()
                    .filter_map(Value::as_array)
                    .map(Vec::len)
                    .sum()
            })
            .unwrap_or(0)
    }

    /// 认领组按事件汇总(只读展示;顺序随 JSON map)。
    pub(crate) fn owned_events(&self) -> Vec<(String, usize)> {
        self.owned
            .as_object()
            .map(|events| {
                events
                    .iter()
                    .filter_map(|(event, groups)| Some((event.clone(), groups.as_array()?.len())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 提交编辑:解析 → SEC-05 校验 → 与磁盘现状合并(认领组原样保留)
    /// → 原子写。失败零写盘。
    pub(crate) fn save_editable(&self, raw: &str) -> Result<(), String> {
        let editable: Value =
            serde_json::from_str(raw).map_err(|error| format!("hooks JSON 语法错误: {error}"))?;
        validate_editable(&editable)?;
        write_editable_at(&self.path, &editable)
    }

    /// 保存成功后重读(认领组可能因安装器并发自愈而变化)。
    pub(crate) fn reload(&self) -> Result<Self, String> {
        Self::load_at(&self.path)
    }
}

/// matcher 组归属:`Some(true)` = 安装器认领(含 slterm-hook 命令);
/// `Some(false)` = 用户/第三方;`None` = 形态非法(加载/保存都拒绝)。
fn group_is_installer_owned(group: &Value) -> Option<bool> {
    let handlers = group.get("hooks")?.as_array()?;
    Some(handlers.iter().any(super::local::is_our_claude_hook))
}

/// SEC-05 语义校验(纯函数):事件名白名单 + handler type == "command" +
/// command 非空。形态非法同样拒绝。
fn validate_editable(hooks: &Value) -> Result<(), String> {
    let events = hooks
        .as_object()
        .ok_or_else(|| "hooks 必须是 JSON 对象".to_owned())?;
    for (event, groups) in events {
        if !CLAUDE_EVENTS.contains(&event.as_str()) {
            return Err(format!(
                "非法 hook 事件名: {event}(仅允许: {})",
                CLAUDE_EVENTS.join(" / ")
            ));
        }
        let groups = groups
            .as_array()
            .ok_or_else(|| format!("事件 {event} 的值必须是数组"))?;
        for group in groups {
            let group = group
                .as_object()
                .ok_or_else(|| format!("事件 {event} 的 matcher 组必须是对象"))?;
            let handlers = group
                .get("hooks")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("事件 {event} 的 matcher 组缺少 hooks 数组"))?;
            for handler in handlers {
                let handler = handler
                    .as_object()
                    .ok_or_else(|| format!("事件 {event} 的 handler 必须是对象"))?;
                if handler.get("type").and_then(Value::as_str) != Some("command") {
                    return Err(format!("事件 {event} 的 handler type 非法(仅允许 command)"));
                }
                let command = handler
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if command.trim().is_empty() {
                    return Err(format!("事件 {event} 的 handler command 必须为非空字符串"));
                }
            }
        }
    }
    Ok(())
}

/// 读根对象:缺失 = 首次创建(空对象);损坏/IO 错误向上传播。
fn read_root(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .map_err(|error| format!("配置文件 JSON 损坏，拒绝编辑: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(error) => Err(error.to_string()),
    }
}

/// 读-改-写主体:锁内重读 → 保留认领组、清掉旧用户组 → 追加编辑组 →
/// 原子写。其余顶层键(hooks 以外的用户配置)原样保留。
fn write_editable_at(path: &Path, editable: &Value) -> Result<(), String> {
    let _lock = crate::atomic_file::try_lock(path)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "hooks 配置文件正被另一个操作占用，请稍后重试".to_owned())?;
    let mut root = read_root(path)?;
    let object = root
        .as_object_mut()
        .ok_or_else(|| "配置文件根必须是 JSON 对象".to_owned())?;
    let hooks = object.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .ok_or_else(|| "hooks 子树必须是 JSON 对象".to_owned())?;
    for (event, groups) in hooks.iter_mut() {
        let groups = groups
            .as_array_mut()
            .ok_or_else(|| format!("事件 {event} 的值必须是数组"))?;
        // 安装器认领组原位保留;旧用户组整段让位给本次编辑结果。
        groups.retain(|group| group_is_installer_owned(group).unwrap_or(true));
    }
    hooks.retain(|_, groups| !groups.as_array().is_some_and(Vec::is_empty));
    let events = editable
        .as_object()
        .ok_or_else(|| "hooks 必须是 JSON 对象".to_owned())?;
    for (event, groups) in events {
        let groups = groups
            .as_array()
            .ok_or_else(|| format!("事件 {event} 的值必须是数组"))?;
        let current = hooks.entry(event.clone()).or_insert_with(|| json!([]));
        let current = current
            .as_array_mut()
            .ok_or_else(|| format!("事件 {event} 的值必须是数组"))?;
        current.extend(groups.iter().cloned());
    }
    let text = serde_json::to_string_pretty(&root).map_err(|error| error.to_string())? + "\n";
    crate::atomic_file::write(path, text.as_bytes()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod editor_tests {
    use super::*;

    const HELPER: &str = "C:/Program Files/slTerminal/runtime/slterm-hook.exe";

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        (directory, path)
    }

    fn write(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn load_marks_marker_groups_read_only_and_keeps_user_domains_editable() {
        let (_directory, path) = fixture();
        write(
            &path,
            r#"{
  "model": "opus",
  "hooks": {
    "Stop": [
      {"matcher": "user", "hooks": [{"type": "command", "command": "user-notifier"}]},
      {"hooks": [{"type": "command", "command": "C:/old/slterm-hook.exe", "args": ["claude"]}]}
    ],
    "SessionStart": [
      {"hooks": [{"type": "command", "command": "C:/new/slterm-hook.exe", "args": ["claude"]}]}
    ]
  }
}"#,
        );
        let document = HooksDocument::load_at(&path).unwrap();
        let editable: Value = serde_json::from_str(&document.editable_json()).unwrap();
        assert_eq!(
            editable["Stop"].as_array().map(Vec::len),
            Some(1),
            "用户组留在编辑域"
        );
        assert_eq!(editable["Stop"][0]["matcher"], json!("user"));
        assert!(
            editable.get("SessionStart").is_none(),
            "纯认领事件不进编辑域"
        );
        assert_eq!(document.owned_group_count(), 2);
        let mut owned_events = document.owned_events();
        owned_events.sort();
        assert_eq!(
            owned_events,
            vec![("SessionStart".to_owned(), 1), ("Stop".to_owned(), 1)],
            "只读展示按事件汇总"
        );
    }

    #[test]
    fn save_preserves_marker_groups_byte_for_byte_and_replaces_user_groups() {
        let (_directory, path) = fixture();
        write(
            &path,
            r#"{"model":"opus","hooks":{"Stop":[
                {"matcher":"user","hooks":[{"type":"command","command":"old-user"}]},
                {"hooks":[{"type":"command","command":"C:/old/slterm-hook.exe","args":["claude"]}]}
            ]}}"#,
        );
        let before = read(&path);
        let marker = before["hooks"]["Stop"][1].clone();
        let document = HooksDocument::load_at(&path).unwrap();
        let edited = json!({
            "Stop": [
                {"matcher": "user", "hooks": [
                    {"type": "command", "command": "new-user"},
                    {"type": "command", "command": "second-user", "timeout": 5}
                ]}
            ],
            "UserPromptSubmit": [
                {"hooks": [{"type": "command", "command": "prompt-user"}]}
            ]
        });
        document.save_editable(&edited.to_string()).unwrap();
        let after = read(&path);
        assert_eq!(after["model"], json!("opus"), "hooks 以外的顶层键原样保留");
        let groups = after["hooks"]["Stop"].as_array().unwrap();
        assert!(groups.contains(&marker), "marker 认领组必须逐字不动");
        let user = groups
            .iter()
            .find(|group| *group != &marker)
            .expect("编辑后的用户组必须在场");
        assert_eq!(user["hooks"][0]["command"], json!("new-user"));
        assert_eq!(user["hooks"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            after["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"],
            json!("prompt-user")
        );
    }

    #[test]
    fn save_rejects_unknown_events_and_non_command_handlers_without_writing() {
        let (_directory, path) = fixture();
        write(&path, r#"{"hooks":{}}"#);
        let before = std::fs::read(&path).unwrap();
        let document = HooksDocument::load_at(&path).unwrap();
        for (invalid, needle) in [
            (
                json!({"NotAnEvent": [{"hooks": [{"type": "command", "command": "x"}]}]}),
                "非法 hook 事件名",
            ),
            (
                json!({"Stop": [{"hooks": [{"type": "http", "url": "https://x"}]}]}),
                "handler type 非法",
            ),
            (
                json!({"Stop": [{"hooks": [{"type": "command", "command": "   "}]}]}),
                "command 必须为非空字符串",
            ),
            (
                json!({"Stop": [{"hooks": [{"command": "x"}]}]}),
                "handler type 非法",
            ),
            (
                json!({"Stop": [{"hooks": [{"type": "command"}]}]}),
                "command 必须为非空字符串",
            ),
            (json!({"Stop": {"hooks": []}}), "值必须是数组"),
        ] {
            let error = document.save_editable(&invalid.to_string()).unwrap_err();
            assert!(error.contains(needle), "错误必须点明原因: {error}");
            assert_eq!(std::fs::read(&path).unwrap(), before, "校验失败不得落盘");
        }
    }

    #[test]
    fn corrupted_documents_are_never_repaired_by_the_editor() {
        let (_directory, path) = fixture();
        write(&path, "broken JSON");
        let error = HooksDocument::load_at(&path).unwrap_err();
        assert!(error.contains("损坏"), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken JSON");
        write(&path, r#"["not", "an", "object"]"#);
        assert!(HooksDocument::load_at(&path).is_err());
    }
}

#[cfg(test)]
mod dual_track_tests {
    use super::*;
    use slterm_settings::AgentHook;

    const HELPER: &str = "C:/Program Files/slTerminal/runtime/slterm-hook.exe";

    /// 双轨联调:设置段写盘 → 安装权威读决策 → 接入/卸载 → inspect 读回
    /// 三态;用户组始终保留,marker 组由安装器独占。
    #[test]
    fn settings_segment_drives_install_and_uninstall_and_inspect_reads_back() {
        let temp = tempfile::tempdir().unwrap();
        let claude_dir = temp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let claude_path = claude_dir.join("settings.json");
        std::fs::write(
            &claude_path,
            r#"{"hooks":{"Stop":[{"matcher":"user","hooks":[{"type":"command","command":"user-notifier"}]}]}}"#,
        )
        .unwrap();
        let settings_path = temp.path().join("settings.json");

        // 轨一:预置偏好(显式开启)经设置写通道落盘。
        crate::settings_store::save_settings_blocking_at(
            &settings_path,
            &[(
                AgentHook::Claude.settings_key(),
                slterm_settings::json::Value::Bool(true),
            )],
        )
        .unwrap();
        let loaded = crate::settings_store::load_settings_at(&settings_path).unwrap();
        let enabled = AgentHook::Claude.enabled(&loaded.data);
        assert!(enabled, "预置偏好必须可被安装权威读回");
        super::super::local::settings::apply_claude_at(&claude_dir, HELPER, enabled).unwrap();

        // inspect 三态:installed + current,且 helper 在位。
        let state = super::super::local::settings::inspect_claude_at(&claude_dir, Some(HELPER));
        assert!(state.installed && !state.needs_repair && !state.helper_missing);
        assert_eq!(state.config_path.as_deref(), Some(claude_path.as_path()));
        // helper 缺失态独立可见(与 installed 双事实分离)。
        let missing = super::super::local::settings::inspect_claude_at(&claude_dir, None);
        assert!(missing.helper_missing && missing.installed);

        // 用户组保留;认领组由安装器维护。
        let root: Value =
            serde_json::from_str(&std::fs::read_to_string(&claude_path).unwrap()).unwrap();
        assert_eq!(
            root["hooks"]["Stop"][0]["hooks"][0]["command"],
            json!("user-notifier")
        );
        let installed = HooksDocument::load_at(&claude_path).unwrap();
        assert!(installed.owned_group_count() >= CLAUDE_EVENTS.len());

        // 轨一:显式关闭 → 卸载路径只移除认领组。
        crate::settings_store::save_settings_blocking_at(
            &settings_path,
            &[(
                AgentHook::Claude.settings_key(),
                slterm_settings::json::Value::Bool(false),
            )],
        )
        .unwrap();
        let loaded = crate::settings_store::load_settings_at(&settings_path).unwrap();
        let enabled = AgentHook::Claude.enabled(&loaded.data);
        assert!(!enabled);
        super::super::local::settings::apply_claude_at(&claude_dir, HELPER, enabled).unwrap();
        let state = super::super::local::settings::inspect_claude_at(&claude_dir, Some(HELPER));
        assert!(!state.installed);
        let root: Value =
            serde_json::from_str(&std::fs::read_to_string(&claude_path).unwrap()).unwrap();
        assert_eq!(
            root["hooks"]["Stop"],
            json!([{"matcher":"user","hooks":[{"type":"command","command":"user-notifier"}]}]),
            "卸载必须恢复 marker 之前的样子"
        );
        assert_eq!(
            HooksDocument::load_at(&claude_path)
                .unwrap()
                .owned_group_count(),
            0
        );
    }

    /// 需修复态:认领组存在但命令是旧 helper 路径时,needs_repair = true;
    /// 编辑器保存不会顺带「修好」它(安装器专属)。
    #[test]
    fn stale_helper_path_reports_needs_repair_and_editor_leaves_it_alone() {
        let temp = tempfile::tempdir().unwrap();
        let claude_dir = temp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let claude_path = claude_dir.join("settings.json");
        std::fs::write(
            &claude_path,
            r#"{"hooks":{"Stop":[
                {"hooks":[{"type":"command","command":"C:/old/slterm-hook.exe","args":["claude"]}]},
                {"hooks":[{"type":"command","command":"user-notifier"}]}
            ]}}"#,
        )
        .unwrap();
        let state = super::super::local::settings::inspect_claude_at(&claude_dir, Some(HELPER));
        assert!(state.installed && state.needs_repair);
        let before: Value =
            serde_json::from_str(&std::fs::read_to_string(&claude_path).unwrap()).unwrap();
        let document = HooksDocument::load_at(&claude_path).unwrap();
        document.save_editable(&document.editable_json()).unwrap();
        let after: Value =
            serde_json::from_str(&std::fs::read_to_string(&claude_path).unwrap()).unwrap();
        assert_eq!(
            after["hooks"]["Stop"][0], before["hooks"]["Stop"][0],
            "编辑器不得触碰 marker 组(修复归安装器)"
        );
    }
}
