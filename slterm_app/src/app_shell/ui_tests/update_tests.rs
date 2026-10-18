//! R7 C27/B52:更新弹窗、提示状态机与安装授权接线(虚拟窗口)。
//!
//! 下载会话经 `update_download::set_status_for_test` 置终态,安装执行经
//! `UpdateInstallRunner` 注入替身——不触真实网络、PowerShell helper 或
//! 安装目录。提示状态默认落在测试构建的进程级临时数据根。

use std::sync::{Arc, Mutex};

use gpui::{TestAppContext, VisualTestContext};
use gpui_component::Root;

use super::open_workspace;
use crate::app_shell::SltermShellEvent;
use crate::app_shell::workspace::SltermWorkspace;
use crate::i18n::Message;
use crate::update_check::{UpdateAsset, UpdateCheckResult};
use crate::update_download::DownloadStatus;

/// 取当前界面语言的文案(提示框按钮标签必须与生产渲染逐字一致)。
fn ui_label(cx: &mut TestAppContext, message: Message) -> String {
    cx.update(|cx| {
        cx.global::<crate::app_shell::config::Settings>()
            .language
            .text(message)
            .to_owned()
    })
}

/// 当前窗口通知数量(错误/成功提示的可观测面)。
fn notification_count(window: &mut VisualTestContext) -> usize {
    window.update(|window, cx| {
        window
            .root::<Root>()
            .flatten()
            .map(|root| {
                let list = root.read(cx).notification.clone();
                list.read(cx).notifications().len()
            })
            .unwrap_or(0)
    })
}

fn update_asset(version: &str) -> UpdateAsset {
    let name = format!("slTerminal-v{version}-windows-x64-setup.exe");
    UpdateAsset {
        version: version.to_owned(),
        name: name.clone(),
        url: format!(
            "https://github.com/LiuShenLan/slTerminal/releases/download/v{version}/{name}"
        ),
        size: Some(42),
        digest: Some("a".repeat(64)),
    }
}

fn update_result(
    version: &str,
    manual_reinstall: bool,
    asset: Option<UpdateAsset>,
) -> UpdateCheckResult {
    UpdateCheckResult {
        current: env!("CARGO_PKG_VERSION").to_owned(),
        latest: version.to_owned(),
        update_available: true,
        manual_reinstall,
        asset,
    }
}

/// 经壳事件入口投递一次更新结果(生产同一条路径)。
fn deliver(
    workspace: &gpui::Entity<SltermWorkspace>,
    window: &mut VisualTestContext,
    result: UpdateCheckResult,
) {
    window.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.handle_shell_event(SltermShellEvent::UpdateAvailable(result), window, cx);
        });
    });
    window.run_until_parked();
}

/// 提示状态文件当前内容(落盘断言端)。
fn prompt_state() -> String {
    std::fs::read_to_string(crate::update_check::update_state_path_for_test()).unwrap_or_default()
}

/// 稍后提醒:打开即记 prompted;选择后写 3 天延迟;同版本再次投递被抑制。
#[gpui::test]
fn update_prompt_remind_later_persists_and_suppresses_repeat(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let version = "9.9.1";
    deliver(
        &workspace,
        &mut window,
        update_result(version, false, Some(update_asset(version))),
    );

    assert!(window.has_pending_prompt(), "更新可用必须弹窗");
    assert!(
        workspace.read_with(cx, |workspace, _| workspace.update_prompt_open_for_test()),
        "弹窗在途标志必须置位"
    );
    assert!(
        !crate::update_check::should_prompt(version),
        "弹窗打开即记 prompted(B52)"
    );

    let remind = ui_label(cx, Message::UpdateRemindLater);
    window.simulate_prompt_answer(&remind);
    window.run_until_parked();
    assert!(!window.has_pending_prompt());
    assert!(
        !workspace.read_with(cx, |workspace, _| workspace.update_prompt_open_for_test()),
        "关闭弹窗后单实例标志必须清除"
    );
    let state = prompt_state();
    assert!(state.contains(version), "提示状态必须落盘: {state}");
    assert!(
        !crate::update_check::should_prompt(version),
        "稍后提醒期间同版本必须抑制"
    );

    // 同版本再次投递:should_prompt 闸门拒绝(单实例标志已清)。
    deliver(
        &workspace,
        &mut window,
        update_result(version, false, Some(update_asset(version))),
    );
    assert!(!window.has_pending_prompt(), "同版本不得重复弹窗");
}

/// 跳过此版本:落盘后同版本不再提示,新版本不受影响。
#[gpui::test]
fn update_prompt_skip_version_persists_and_suppresses_repeat(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let version = "9.9.2";
    deliver(
        &workspace,
        &mut window,
        update_result(version, false, Some(update_asset(version))),
    );
    assert!(window.has_pending_prompt());

    let skip = ui_label(cx, Message::UpdateSkipVersion);
    window.simulate_prompt_answer(&skip);
    window.run_until_parked();
    assert!(!window.has_pending_prompt());
    let state = prompt_state();
    assert!(state.contains(version), "跳过必须落盘: {state}");
    assert!(
        !crate::update_check::should_prompt(version),
        "被跳过的版本不得再提示"
    );
    assert!(
        crate::update_check::should_prompt("9.9.20"),
        "跳过只影响该版本"
    );

    deliver(
        &workspace,
        &mut window,
        update_result(version, false, Some(update_asset(version))),
    );
    assert!(!window.has_pending_prompt(), "同版本不得重复弹窗");
}

/// 弹窗在途时另一个版本的事件必须被单实例闸门丢弃,且不写该版本状态。
#[gpui::test]
fn update_prompt_is_single_instance_while_open(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let first = "9.9.3";
    let second = "9.9.4";
    deliver(
        &workspace,
        &mut window,
        update_result(first, false, Some(update_asset(first))),
    );
    assert!(window.has_pending_prompt());

    deliver(
        &workspace,
        &mut window,
        update_result(second, false, Some(update_asset(second))),
    );
    assert!(
        crate::update_check::should_prompt(second),
        "被丢弃的事件不得提前标记 prompted"
    );

    let remind = ui_label(cx, Message::UpdateRemindLater);
    window.simulate_prompt_answer(&remind);
    window.run_until_parked();
    assert!(
        !window.has_pending_prompt(),
        "第二事件必须被 update_prompt_open 闸门丢弃,不得排队"
    );
}

/// 手动重装/无可安装资产:第一项为「打开下载页」,点击落到官方 releases 页。
#[gpui::test]
fn update_prompt_manual_reinstall_opens_download_page(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let version = "9.9.9";
    deliver(
        &workspace,
        &mut window,
        update_result(version, true, Some(update_asset(version))),
    );
    assert!(window.has_pending_prompt());

    let open = ui_label(cx, Message::UpdateOpenDownloadPage);
    window.simulate_prompt_answer(&open);
    window.run_until_parked();

    assert_eq!(
        cx.opened_url(),
        Some(crate::update_check::RELEASES_PAGE.to_owned()),
        "手动重装提示必须打开官方下载页"
    );
    assert!(
        !workspace.read_with(cx, |workspace, _| workspace.update_prompt_open_for_test()),
        "打开下载页后必须清除单实例标志"
    );
}

/// 下载失败(资产经下载器二次校验拒绝):回到窗口错误通知,不进入安装授权。
#[gpui::test]
fn update_available_download_failure_reports_notification(cx: &mut TestAppContext) {
    crate::update_download::reset_session();
    let (workspace, mut window) = open_workspace(cx);
    let version = "9.9.5";
    let mut asset = update_asset(version);
    // 按钮决策只看 digest,仍进入「下载并安装」;下载器的官方 URL 合同拒绝。
    asset.url = "https://example.invalid/untrusted.exe".into();
    deliver(
        &workspace,
        &mut window,
        update_result(version, false, Some(asset)),
    );

    let download = ui_label(cx, Message::UpdateDownloadAndInstall);
    window.simulate_prompt_answer(&download);
    window.run_until_parked();

    assert!(!window.has_pending_prompt(), "下载失败不得打开安装授权");
    assert!(
        !workspace.read_with(cx, |workspace, _| workspace.update_prompt_open_for_test()),
        "失败后必须清除单实例标志"
    );
    assert!(
        notification_count(&mut window) >= 1,
        "下载失败必须给窗口错误通知"
    );
}

/// 安装授权前失败:前台快照已采集并交给后台,窗口保持打开并显示错误通知。
#[gpui::test]
fn update_install_failure_keeps_window_open_and_reports_notification(cx: &mut TestAppContext) {
    crate::update_download::reset_session();
    let (workspace, mut window) = open_workspace(cx);
    let version = "9.9.6";
    let asset = update_asset(version);
    stage_ready(&asset);
    open_install_authorization(&workspace, &mut window, asset);

    let seen: Arc<Mutex<Option<(String, usize)>>> = Arc::new(Mutex::new(None));
    let recorder = seen.clone();
    workspace.update(cx, |workspace, _| {
        workspace.set_update_install_runner_for_test(Arc::new(move |asset, sessions| {
            *recorder.lock().unwrap() = Some((asset.version.clone(), sessions.len()));
            Err("模拟安装失败".to_owned())
        }));
    });

    let install = ui_label(cx, Message::UpdateInstallNow);
    window.simulate_prompt_answer(&install);
    window.run_until_parked();

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some((version.to_owned(), 1)),
        "前台必须先拍工作区快照并交给后台"
    );
    assert!(
        !workspace.read_with(cx, |workspace, _| workspace.update_install_exit_requested()),
        "安装失败不得关窗"
    );
    assert!(
        !workspace.read_with(cx, |workspace, _| workspace.update_prompt_open_for_test()),
        "失败后必须清除单实例标志"
    );
    assert!(
        notification_count(&mut window) >= 1,
        "安装失败必须给窗口错误通知"
    );
}

/// 安装授权成功:终局落盘后请求退出(虚拟窗口记录请求,不真实移除)。
#[gpui::test]
fn update_install_success_captures_snapshot_and_requests_exit(cx: &mut TestAppContext) {
    crate::update_download::reset_session();
    let (workspace, mut window) = open_workspace(cx);
    let version = "9.9.7";
    let asset = update_asset(version);
    stage_ready(&asset);
    open_install_authorization(&workspace, &mut window, asset);

    let seen: Arc<Mutex<Option<(String, usize)>>> = Arc::new(Mutex::new(None));
    let recorder = seen.clone();
    workspace.update(cx, |workspace, _| {
        workspace.set_update_install_runner_for_test(Arc::new(move |asset, sessions| {
            *recorder.lock().unwrap() = Some((asset.version.clone(), sessions.len()));
            Ok(())
        }));
    });

    let install = ui_label(cx, Message::UpdateInstallNow);
    window.simulate_prompt_answer(&install);
    window.run_until_parked();

    assert_eq!(
        seen.lock().unwrap().clone(),
        Some((version.to_owned(), 1)),
        "成功路径同样必须先采集前台快照"
    );
    assert!(
        workspace.read_with(cx, |workspace, _| workspace.update_install_exit_requested()),
        "成功后必须走终局落盘 + 退出链"
    );
    assert!(!window.has_pending_prompt());
}

/// 「下次启动安装」失败(会话已失效):错误通知并清除单实例标志。
#[gpui::test]
fn update_install_next_launch_failure_reports_notification(cx: &mut TestAppContext) {
    crate::update_download::reset_session();
    let (workspace, mut window) = open_workspace(cx);
    let version = "9.9.8";
    let asset = update_asset(version);
    stage_ready(&asset);
    open_install_authorization(&workspace, &mut window, asset);
    // 预约要求下载会话仍为 Ready;清掉后必须走失败通知分支。
    crate::update_download::reset_session();

    let next_launch = ui_label(cx, Message::UpdateInstallNextLaunch);
    window.simulate_prompt_answer(&next_launch);
    window.run_until_parked();

    assert!(
        !workspace.read_with(cx, |workspace, _| workspace.update_prompt_open_for_test()),
        "预约失败后必须清除单实例标志"
    );
    assert!(
        notification_count(&mut window) >= 1,
        "预约失败必须给窗口错误通知"
    );
}

/// 预置下载就绪态(不再执行网络;安装执行由替身接管)。
fn stage_ready(asset: &UpdateAsset) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(&asset.name);
    crate::update_download::set_status_for_test(asset, DownloadStatus::Ready { path, bytes: 42 });
}

/// 走真实弹窗路径推进到第二层安装授权。
fn open_install_authorization(
    workspace: &gpui::Entity<SltermWorkspace>,
    window: &mut VisualTestContext,
    asset: UpdateAsset,
) {
    let version = asset.version.clone();
    deliver(
        workspace,
        window,
        update_result(&version, false, Some(asset)),
    );
    assert!(window.has_pending_prompt(), "第一层更新弹窗必须出现");
    let download = ui_label(window, Message::UpdateDownloadAndInstall);
    window.simulate_prompt_answer(&download);
    window.run_until_parked();
    assert!(window.has_pending_prompt(), "下载就绪必须打开安装授权");
    assert!(
        workspace.read_with(window, |workspace, _| workspace
            .update_prompt_open_for_test()),
        "安装授权在途时单实例标志必须保持"
    );
}
