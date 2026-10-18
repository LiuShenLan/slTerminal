//! 单窗口会话快照与冷恢复(上游 `session_recovery` 裁剪)。
//!
//! 快照侧只收 Terminal tab,叶数据经 `session_restore` 转换对落成持久化
//! 树;恢复侧先按 DFS 序逐叶 spawn,再用同一配对建树——「panes 的 id 集合
//! == 树叶集合」不变式在恢复路径同样成立。
//!
//! 启动序:
//! `load → should_restore →(否)quarantine → mark_boot_attempt → 逐 tab
//! restore_tab → 首帧`。窗口尺寸/侧栏宽度在开窗定形前应用(D05-1 回放):
//! 本模块只产出 [`StartupPlan`],定形消费在 `windowing`。
//!
//! 恢复注入的 pane 一律走 `new_pane` 唯一路径(注册表/AgentHub 写入点
//! 唯一);`zoomed` 不进快照,恢复恒 `false`。

use std::path::{Path, PathBuf};

use gpui::{App, Window, WindowBounds};

use super::sidebar::SidebarState;
use super::{SltermWorkspace, TabEntry, TabMeta, WorkspaceTab};
use crate::gpui_terminal::view::TerminalLaunch;
use crate::session::{LaunchSession, LayoutSession, Session, TabSession, WindowState};
use slterm_split::{PaneId, TabId};

/// 启动恢复计划:开窗前算好,`SltermWorkspace::new` 只消费不决策。
///
/// `session` 仅在「应恢复」时为 `Some`;窗口尺寸/侧栏宽度即使不恢复 tab
/// 也照常回放(D05-1)。
#[derive(Debug, Clone)]
pub(crate) struct StartupPlan {
    pub(crate) session: Option<Session>,
    /// 侧栏宽度/开合随现场恢复(D05-3)。
    pub(crate) sidebar: SidebarState,
    /// 上次窗口尺寸与最大化态(D05-1)。
    pub(crate) window: Option<WindowState>,
    /// 上次是异常退出(仅用于恢复提示文案)。
    pub(crate) crashed: bool,
    /// 断路器跳闸后隔离出来的诊断文件路径。
    pub(crate) quarantined: Option<PathBuf>,
}

impl StartupPlan {
    /// 全新启动:无会话、默认侧栏、无窗口回放。
    pub(crate) fn fresh() -> Self {
        Self {
            session: None,
            sidebar: SidebarState::default(),
            window: None,
            crashed: false,
            quarantined: None,
        }
    }
}

/// 恢复结果一次性汇总文案:
/// 恢复成功按标签数提示(崩溃现场多一句来源说明);断路器隔离给出现场
/// 路径;无恢复且无隔离 = None(全新启动不打扰)。
pub(crate) fn recovery_notice(
    restored_tabs: Option<usize>,
    crashed: bool,
    quarantined: Option<&Path>,
    language: crate::i18n::UiLanguage,
) -> Option<String> {
    use crate::i18n::Message;

    match restored_tabs {
        Some(tabs) if tabs > 0 => {
            let count = tabs.to_string();
            let message = if crashed {
                Message::NotifyRecoveryCrashed
            } else {
                Message::NotifyRecoveryRestored
            };
            Some(language.format(message, &[("count", &count)]))
        }
        _ => quarantined.map(|path| {
            language.format(
                Message::NotifyRecoveryQuarantined,
                &[("path", &path.display().to_string())],
            )
        }),
    }
}

/// 生产启动入口:数据目录 `session.json`。
pub(crate) fn prepare_startup(restore_tabs: bool) -> StartupPlan {
    prepare_startup_at(&crate::session::session_path(), restore_tabs)
}

/// 测试/显式路径形态。`restore_tabs=false`(CLI 显式 cwd 或 `-e` 启动)
/// 只回放窗口几何,不计数、不隔离,把完整会话留给下一次默认启动。
pub(crate) fn prepare_startup_at(path: &Path, restore_tabs: bool) -> StartupPlan {
    let Some(mut session) = crate::session::load_from(path) else {
        return StartupPlan::fresh();
    };
    let mut plan = StartupPlan {
        session: None,
        sidebar: session.sidebar,
        window: session.window,
        crashed: false,
        quarantined: None,
    };
    if !restore_tabs {
        return plan;
    }
    if !crate::session::should_restore(&session) {
        if !session.tabs.is_empty() {
            // 连续几次启动都没活到首次自动保存:挪走现场(唯一诊断材料),
            // 本次干净启动。一秒后的自动保存会盖掉原路径,所以必须挪。
            plan.quarantined = crate::session::quarantine_at(path);
            match &plan.quarantined {
                Some(quarantined) => log::warn!(
                    "session breaker tripped; crashed scene moved to {}",
                    quarantined.display()
                ),
                None => log::warn!("session breaker tripped; quarantine move failed"),
            }
        }
        return plan;
    }
    plan.crashed = crate::session::was_crash(&session);
    // 先计数落盘再恢复:恢复过程中崩溃会被断路器记到。
    if let Err(error) = crate::session::mark_boot_attempt_at(path, &mut session) {
        log::warn!("could not persist boot attempt count: {error}");
    }
    plan.session = Some(session);
    plan
}

/// 窗口当前尺寸/最大化态。最大化时 `window_bounds()` 携带的是还原尺寸,
/// 正好是回放 D05-1 需要的「还原后大小 + 最大化」二元组。
pub(crate) fn capture_window_state(window: &Window) -> WindowState {
    let bounds = window.window_bounds();
    let restore = bounds.get_bounds();
    WindowState {
        width: pixels_to_u32(restore.size.width),
        height: pixels_to_u32(restore.size.height),
        maximized: matches!(bounds, WindowBounds::Maximized(_)),
    }
}

fn pixels_to_u32(value: gpui::Pixels) -> u32 {
    (f32::from(value).max(1.0).round() as u32).max(1)
}

impl SltermWorkspace {
    /// 当前工作区 → session 快照。`window_state` 由调用方从真实窗口拍取
    /// (1 Hz 泵与导出路径共用同一快照函数;None 只出现在无窗口的纯函数
    /// 用例里)。
    pub(crate) fn snapshot_session(&self, window_state: Option<WindowState>, cx: &App) -> Session {
        let mut tabs = Vec::new();
        let mut active_out = 0usize;
        for (ix, entry) in self.tabs.iter().enumerate() {
            // 快照只收 Terminal tab:设置/文档类 tab 不进会话。
            let WorkspaceTab::Terminal {
                panes,
                tree,
                focused,
                ..
            } = &entry.tab
            else {
                continue;
            };
            if ix == self.active {
                active_out = tabs.len();
            }
            let leaf_data = |pane_id: PaneId| {
                let Some(pane) = panes.iter().find(|pane| pane.id == pane_id) else {
                    return (String::new(), None, None, None);
                };
                (
                    pane_cwd(pane, cx),
                    self.agent_hub.session_agent(pane_id),
                    pane.launch.clone(),
                    pane.custom_name.clone(),
                )
            };
            let layout = crate::app_shell::session_restore::layout_from_tree(tree, &leaf_data);
            let cwd = panes
                .iter()
                .find(|pane| pane.id == *focused)
                .map(|pane| pane_cwd(pane, cx))
                .unwrap_or_default();
            let active_pane = tree
                .leaves()
                .iter()
                .position(|pane_id| pane_id == focused)
                .unwrap_or(0);
            tabs.push(TabSession {
                cwd,
                custom_name: entry.meta.custom_name.clone(),
                color: entry.meta.color,
                launch: entry.meta.launch.clone(),
                layout: Some(layout),
                active_pane,
            });
        }
        let mut session = Session::new(active_out, tabs);
        session.window = window_state;
        session.sidebar = self.sidebar;
        session
    }

    /// 整窗导出(同步形态):与 `session.json` 同一 schema 的 pretty 打印。
    /// 只供测试与后续 CLI 导出复用;UI 路径走
    /// [`Self::export_workspace_task`],避免在前台线程写盘。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn export_workspace(
        &self,
        path: &Path,
        window: &Window,
        cx: &App,
    ) -> std::io::Result<()> {
        let session = self.snapshot_session(Some(capture_window_state(window)), cx);
        crate::session::save_to(path, &session)
    }

    /// 导出 UI 路径:前台只拍内存快照,写盘在后台执行器完成。
    pub(crate) fn export_workspace_task(
        &self,
        path: &Path,
        window: &Window,
        cx: &mut gpui::Context<'_, Self>,
    ) -> gpui::Task<std::io::Result<()>> {
        let session = self.snapshot_session(Some(capture_window_state(window)), cx);
        let path = path.to_path_buf();
        cx.background_executor()
            .spawn(async move { crate::session::save_to(&path, &session) })
    }

    /// 逐 tab 恢复;至少恢复出一个 tab 才算成功。调用方在恢复失败时回退
    /// 到正常新建路径。
    pub(crate) fn try_restore_session(
        &mut self,
        session: &Session,
        window: &mut Window,
        cx: &mut gpui::Context<'_, Self>,
    ) -> bool {
        let mut restored = 0usize;
        for tab in &session.tabs {
            if self.restore_tab(tab, window, cx) {
                restored += 1;
            }
        }
        if restored == 0 {
            return false;
        }
        self.active = session.active_tab.min(self.tabs.len().saturating_sub(1));
        self.focus_active(window, cx);
        self.sync_window_title(window, cx);
        log::info!(
            "restored {restored} terminal tab(s){}",
            if crate::session::was_crash(session) {
                " from a crashed session"
            } else {
                ""
            }
        );
        cx.notify();
        true
    }

    /// 按保存的逐 pane 启动环境重建一个 tab:先逐叶 spawn(DFS 序),再建树
    /// 并插入尾部(恢复期不套新 tab 插入策略)。
    fn restore_tab(
        &mut self,
        tab: &TabSession,
        window: &mut Window,
        cx: &mut gpui::Context<'_, Self>,
    ) -> bool {
        let layout = tab
            .layout
            .clone()
            .unwrap_or_else(|| LayoutSession::single_pane(tab.cwd.clone()));
        let saved_launch = tab.launch.clone().unwrap_or(LaunchSession::Default);
        // tab.cwd 是焦点 pane 目录;它失效时用树的首叶目录做种子回退。
        let tab_cwd = crate::session::valid_dir(&tab.cwd)
            .or_else(|| crate::session::valid_dir(layout.first_cwd()));
        let leaves = layout.leaves();
        let mut panes = Vec::with_capacity(layout.pane_count());
        for (index, leaf) in leaves.iter().enumerate() {
            let Some(data) = leaf.pane_data() else {
                continue;
            };
            // 目录消失回退下一级:叶 cwd → tab cwd → 默认 cwd(None)。
            let leaf_cwd = crate::session::valid_dir(data.cwd).or_else(|| tab_cwd.clone());
            let launch_session = data.launch.cloned().unwrap_or_else(|| {
                if index == 0 {
                    saved_launch.clone()
                } else {
                    // 旧文件未带逐 pane 描述:非首叶回默认本地启动。
                    LaunchSession::Default
                }
            });
            let launch = terminal_launch_from_session(&launch_session, leaf_cwd);
            let mut pane = self.new_pane(self.initial_grid, launch, window, cx);
            pane.custom_name = data.custom_name.map(str::to_owned);
            pane.launch = Some(launch_session);
            if let Some(agent) = data.agent {
                // 恢复目标先入宿主:快照在 hook 确认前也不会丢失接续身份。
                self.agent_hub.seed_session(pane.id, agent.clone());
                pane.view
                    .update(cx, |view, cx| view.restore_agent(agent, cx));
            }
            panes.push(pane);
        }
        if panes.is_empty() {
            return false;
        }

        // alloc 按 DFS 序吐已创建 pane 的 id——恢复注入的配对权威。
        let mut next = panes.iter().map(|pane| pane.id);
        let mut exhausted = false;
        let (tree, ids) = crate::app_shell::session_restore::tree_from_layout(&layout, &mut || {
            next.next().unwrap_or_else(|| {
                exhausted = true;
                PaneId(0)
            })
        });
        if exhausted || ids.len() != panes.len() {
            // 建树挂不上 = 失败路径:立即回收已 spawn 的 pane,不留孤儿 PTY。
            for pane in &panes {
                pane.view.read(cx).shutdown();
            }
            for pane in &panes {
                self.forget_pane(pane.id, cx);
            }
            log::warn!("could not rebuild the saved split tree; restored tab discarded");
            return false;
        }

        let focused = panes
            .get(tab.active_pane)
            .or_else(|| panes.first())
            .map(|pane| pane.id)
            .unwrap_or(PaneId(0));
        let tab_id = TabId(self.next_tab);
        self.next_tab = self.next_tab.saturating_add(1);
        self.tabs.push(TabEntry {
            id: tab_id,
            tab: WorkspaceTab::Terminal {
                panes,
                tree,
                focused,
                // 运行时态不进会话:恢复恒非缩放。
                zoomed: false,
            },
            meta: TabMeta {
                custom_name: tab.custom_name.clone(),
                color: tab.color,
                // 保留保存时的形态:老文件 `None` 不在恢复后被改写成
                // `Some(Default)`,首拍快照与导出保持字段级稳定。
                launch: tab.launch.clone(),
            },
        });
        true
    }
}

/// 持久化启动身份 → 运行时启动参数。
///
/// `Profile` 走显式命令通道(`explicit_command = true`):快照里冻结的是
/// 建 tab 时 core 解析后的完整命令,恢复必须按同一命令重建(`-e` 的任意
/// 程序也要能复活);Schema 是本机用户态持久化输入,与远程/hook 注入的
/// 不可信 id 是两条路——agent 恢复命令另经 `AgentSession::resume_command`
/// 校验(SEC-01 豁免论证同 `-e` 通道)。
fn terminal_launch_from_session(launch: &LaunchSession, cwd: Option<PathBuf>) -> TerminalLaunch {
    match launch {
        LaunchSession::Default => TerminalLaunch::Local {
            cwd,
            shell: None,
            shell_name: None,
            explicit_command: false,
        },
        LaunchSession::Profile {
            name,
            command,
            args,
            cwd: launch_cwd,
            ..
        } => {
            let cwd = launch_cwd
                .as_deref()
                .and_then(crate::session::valid_dir)
                .or(cwd);
            TerminalLaunch::Local {
                cwd,
                shell: Some(slterm_terminal::tty::Shell::new(
                    command.clone(),
                    args.clone(),
                )),
                shell_name: Some(name.clone()),
                explicit_command: true,
            }
        }
    }
}

/// pane 的持久化 cwd:终端报告优先,shell 尚未上报时回退启动身份里的
/// 出生目录(恢复后首拍快照不会把目录丢成空串)。
pub(super) fn pane_cwd(pane: &super::TerminalPane, cx: &App) -> String {
    let reported = pane.view.read(cx).cwd.clone();
    if !reported.is_empty() {
        return reported;
    }
    match &pane.launch {
        Some(LaunchSession::Profile { cwd: Some(cwd), .. }) => cwd.clone(),
        _ => String::new(),
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
impl SltermWorkspace {
    /// 测试专用:注入会话写函数(观察自动保存快照;不触真实数据目录)。
    pub(crate) fn set_session_writer_for_test(
        &mut self,
        writer: super::session_persistence::SessionWriter,
    ) {
        self.session_persistence =
            super::session_persistence::SessionPersistence::with_writer(writer, false);
    }

    /// 测试专用:手动驱动一拍自动保存(等价 1 Hz 泵的一拍)。
    pub(crate) fn autosave_tick_for_test(&mut self, cx: &mut gpui::Context<'_, Self>) {
        self.autosave_tick(cx);
    }

    /// 测试专用:快照当前工作区(窗口态由调用方拍取)。
    pub(crate) fn snapshot_for_test(&self, window_state: Option<WindowState>, cx: &App) -> Session {
        self.snapshot_session(window_state, cx)
    }

    /// 测试专用:第 `tab_ix` 个 tab 第 `pane_ix` 个 pane 的 cwd。
    pub(crate) fn pane_cwd_for_test(
        &self,
        tab_ix: usize,
        pane_ix: usize,
        cx: &App,
    ) -> Option<String> {
        self.tab_panes(tab_ix)
            .nth(pane_ix)
            .map(|pane| pane_cwd(pane, cx))
    }

    /// 测试专用:pane 的会话身份(恢复注入/快照的断言端)。
    pub(crate) fn agent_session_for_test(
        &self,
        pane_id: PaneId,
    ) -> Option<crate::session::AgentSession> {
        self.agent_hub.session_agent(pane_id)
    }
}

#[cfg(test)]
mod session_recovery_tests {
    use super::*;
    use std::fs;

    fn workspace_json(tabs: &str, boot_attempts: u32, clean_exit: bool) -> String {
        format!(
            r#"{{
              "version": 1,
              "boot_attempts": {boot_attempts},
              "active_tab": 0,
              "tabs": [{tabs}],
              "window": {{ "width": 1280, "height": 720, "maximized": true }},
              "sidebar": {{ "collapsed": true, "active_view_top": "nav",
                            "active_view_bottom": null, "width": 312.0 }},
              "clean_exit": {clean_exit}
            }}"#
        )
    }

    const ONE_TAB: &str = r#"{ "cwd": "D:/work", "active_pane": 0,
                              "layout": { "kind": "pane", "cwd": "D:/work" } }"#;

    fn write_session(path: &Path, text: &str) {
        fs::write(path, text).unwrap();
    }

    /// 缺失/损坏都按无会话启动;损坏文件不隔离(会被首拍自动保存覆盖)。
    #[test]
    fn missing_and_corrupt_sessions_start_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("session.json");
        let plan = prepare_startup_at(&missing, true);
        assert!(plan.session.is_none());
        assert!(plan.quarantined.is_none());
        assert_eq!(plan.sidebar, SidebarState::default());
        assert_eq!(plan.window, None);

        let corrupt = dir.path().join("corrupt.json");
        fs::write(&corrupt, b"{not json").unwrap();
        let plan = prepare_startup_at(&corrupt, true);
        assert!(plan.session.is_none());
        assert!(corrupt.exists(), "损坏文件不挪走");
    }

    /// 空会话(一路关标签关干净)不恢复、不隔离;窗口/侧栏几何仍回放。
    #[test]
    fn empty_sessions_do_not_restore_and_keep_their_geometry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        write_session(&path, &workspace_json("", 0, true));
        let plan = prepare_startup_at(&path, true);
        assert!(plan.session.is_none());
        assert!(plan.quarantined.is_none());
        assert_eq!(plan.window.map(|window| window.width), Some(1280));
        assert!(plan.sidebar.collapsed);
        assert_eq!(plan.sidebar.width, 312.0);
        assert!(path.exists(), "空会话不是崩溃现场");
    }

    /// 断路器跳闸:现场挪到 session.crashed.json,本次干净启动。
    #[test]
    fn the_breaker_quarantines_the_crashed_scene() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        write_session(
            &path,
            &workspace_json(ONE_TAB, crate::session::MAX_BOOT_ATTEMPTS, false),
        );
        let plan = prepare_startup_at(&path, true);
        assert!(plan.session.is_none());
        assert!(!path.exists(), "原路径必须清空");
        let quarantined = plan.quarantined.expect("必须产出隔离文件");
        assert_eq!(quarantined, dir.path().join("session.crashed.json"));
        assert!(quarantined.exists());
    }

    /// 应恢复时先计数落盘,再交出会话;崩溃标记来自 clean_exit。
    #[test]
    fn a_restorable_session_is_counted_before_it_is_handed_over() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        write_session(&path, &workspace_json(ONE_TAB, 0, false));
        let plan = prepare_startup_at(&path, true);
        let session = plan.session.expect("必须恢复");
        assert_eq!(session.tabs.len(), 1);
        assert!(plan.crashed, "clean_exit=false 是崩溃现场");
        let on_disk = crate::session::load_from(&path).unwrap();
        assert_eq!(on_disk.boot_attempts, 1, "恢复前先计数落盘");

        // 正常退出文件照常恢复,但不是崩溃提示。
        write_session(&path, &workspace_json(ONE_TAB, 0, true));
        let plan = prepare_startup_at(&path, true);
        assert!(plan.session.is_some());
        assert!(!plan.crashed);
    }

    /// 持久化启动身份 → 运行时参数:Profile 走显式命令通道并保留完整
    /// 命令;Default 交回 core 探测链。
    #[test]
    fn launch_sessions_map_to_the_runtime_launch_shape() {
        let dir = tempfile::tempdir().unwrap();
        let leaf_cwd = dir.path().to_path_buf();

        let default_launch =
            terminal_launch_from_session(&LaunchSession::Default, Some(leaf_cwd.clone()));
        let TerminalLaunch::Local {
            cwd,
            shell,
            shell_name,
            explicit_command,
        } = default_launch;
        assert_eq!(cwd, Some(leaf_cwd.clone()));
        assert!(shell.is_none(), "Default 交回 core 探测链");
        assert_eq!(shell_name, None);
        assert!(!explicit_command);

        let profile = LaunchSession::Profile {
            name: "pwsh".into(),
            command: "pwsh.exe".into(),
            args: vec!["-NoLogo".into()],
            // 失效的 profile cwd 回退到叶目录。
            cwd: Some(dir.path().join("missing").to_string_lossy().into_owned()),
            shell_id: None,
        };
        let profile_launch = terminal_launch_from_session(&profile, Some(leaf_cwd.clone()));
        let TerminalLaunch::Local {
            cwd,
            shell,
            shell_name,
            explicit_command,
        } = profile_launch;
        let shell = shell.expect("Profile 必须带完整命令");
        assert_eq!(cwd, Some(leaf_cwd));
        assert_eq!(shell.program(), "pwsh.exe");
        assert_eq!(shell.args(), &["-NoLogo".to_owned()]);
        assert_eq!(shell_name.as_deref(), Some("pwsh"));
        assert!(explicit_command, "Profile 按显式命令通道重建");
    }

    /// 恢复汇总文案(A10):普通恢复/崩溃恢复/断路器隔离三态,全新启动
    /// 静默;零标签不生成提示。
    #[test]
    fn recovery_notice_summarizes_restore_outcomes_and_stays_quiet_on_fresh_start() {
        use crate::i18n::UiLanguage;

        assert_eq!(
            recovery_notice(Some(3), false, None, UiLanguage::EnUs).as_deref(),
            Some("Restored 3 tab(s).")
        );
        assert_eq!(
            recovery_notice(Some(2), true, None, UiLanguage::EnUs).as_deref(),
            Some("Restored 2 tab(s) after an unclean shutdown.")
        );
        assert_eq!(
            recovery_notice(
                None,
                false,
                Some(Path::new("D:/data/session.crashed.json")),
                UiLanguage::ZhCn
            )
            .as_deref(),
            Some("连续多次启动未完成恢复，已跳过；现场保存在 D:/data/session.crashed.json")
        );
        assert_eq!(recovery_notice(None, false, None, UiLanguage::EnUs), None);
        assert_eq!(
            recovery_notice(Some(0), false, None, UiLanguage::EnUs),
            None
        );
    }

    /// CLI 显式启动(cwd/-e)不碰恢复:不计数、不隔离、不交会话。
    #[test]
    fn explicit_startup_only_replays_geometry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        write_session(
            &path,
            &workspace_json(ONE_TAB, crate::session::MAX_BOOT_ATTEMPTS, false),
        );
        let plan = prepare_startup_at(&path, false);
        assert!(plan.session.is_none());
        assert!(plan.quarantined.is_none());
        assert!(path.exists(), "显式启动不得挪走会话");
        assert_eq!(
            crate::session::load_from(&path).unwrap().boot_attempts,
            crate::session::MAX_BOOT_ATTEMPTS,
            "显式启动不计断路器"
        );
        assert_eq!(plan.window.map(|window| window.height), Some(720));
    }
}
