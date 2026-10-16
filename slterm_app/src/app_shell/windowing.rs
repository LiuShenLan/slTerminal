//! 开窗链:窗口选项(116×30 反推尺寸/标题栏/app_id)与启动变体分派。
//!
//! M3 单窗单实例(产品定位):`open_initial_window` 只在启动链调一次;
//! 窗口角色单形态(QuickTerminal 归 09),会话恢复变体(RestoreOrDefault/
//! RestoreUpdate/Empty)归 05 篇 M5.5,恢复消费点在 `WorkspaceStartup` 上
//! 预留变体位而不立死类型。

pub(super) mod startup_geometry;

use std::path::PathBuf;
use std::sync::mpsc;

use gpui::{
    App, AppContext as _, Bounds, WindowBackgroundAppearance, WindowBounds, WindowOptions, px, size,
};
use gpui_component::{Root, TitleBar};

use super::SltermShellEvent;
use super::workspace::SltermWorkspace;
use crate::brand;
use crate::cli::Program;
use crate::platform::startup::first_frame;

/// 首窗的启动形态(13 篇 M3 两变体)。
pub(crate) enum WorkspaceStartup {
    /// 新终端(可带显式 cwd)。
    NewTerminal { cwd: Option<PathBuf> },
    /// 新终端并执行启动命令(`-e` 尾捕获)。
    LaunchTerminal {
        cwd: Option<PathBuf>,
        command: Program,
    },
}

/// 窗口定形唯一函数(13 篇缝合点 11:M5.5 回放 = 定形前覆盖 preferred_size,
/// 不改调用序)。M3 单形态 = Regular。
pub(crate) fn workspace_window_options(cx: &App, focus: bool) -> WindowOptions {
    let preferred = startup_geometry::preferred_size(cx, startup_geometry::M3_SIDEBAR_WIDTH)
        .unwrap_or_else(|| size(px(1080.0), px(720.0)));
    let bounds = cx.primary_display().map_or_else(
        || Bounds::centered(None, preferred, cx),
        |display| {
            let visible = display.visible_bounds();
            let fitted = preferred.min(&visible.size);
            Bounds::new(
                gpui::point(
                    visible.origin.x + (visible.size.width - fitted.width) / 2.0,
                    visible.origin.y + (visible.size.height - fitted.height) / 2.0,
                ),
                fitted,
            )
        },
    );
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(760.0), px(540.0)).min(&bounds.size)),
        titlebar: Some(TitleBar::title_bar_options()),
        app_id: Some(brand::WINDOWS_APP_ID.to_owned()),
        // 仅暗色 + 不透明底:壁纸/acrylic 归 09 篇。
        window_background: WindowBackgroundAppearance::Opaque,
        focus,
        ..Default::default()
    }
}

/// 启动链调一次的开窗入口:命令行 `-e` → LaunchTerminal;有 cwd →
/// NewTerminal;两者皆无仍是 NewTerminal(M3 无会话恢复,恢复变体归 05)。
pub(crate) fn open_initial_window(
    cx: &mut App,
    shell_rx: mpsc::Receiver<SltermShellEvent>,
    initial_cwd: Option<PathBuf>,
    initial_command: Option<Program>,
) {
    let startup = match initial_command {
        Some(command) => WorkspaceStartup::LaunchTerminal {
            cwd: initial_cwd,
            command,
        },
        None => WorkspaceStartup::NewTerminal { cwd: initial_cwd },
    };
    let mut options = workspace_window_options(cx, true);
    // 首帧黑窗/半构窗口不曝光:先同步画一帧再 activate(first_frame 因果链)。
    let deferred_show = first_frame::defer_show(&mut options);
    let result = cx.open_window(options, move |window, cx| {
        window.set_window_title(brand::NAME);
        let workspace = cx.new(|cx| SltermWorkspace::new(window, shell_rx, startup, cx));
        cx.new(|cx| Root::new(workspace, window, cx))
    });
    match result {
        Ok(handle) if deferred_show => first_frame::present_then_show(handle.into(), cx),
        Ok(_) => {}
        Err(error) => {
            log::error!("failed to open main window: {error}");
            super::try_write_stderr(format_args!("failed to open main window: {error}"));
        }
    }
}

#[cfg(test)]
mod windowing_tests {
    use super::*;

    /// 启动命令必须原样保留程序与参数(逐字,不经 shell 二次解析)。
    #[test]
    fn explicit_program_is_kept_with_its_arguments_and_directory() {
        let command = Program::WithArgs {
            program: "shell.exe".into(),
            args: vec!["--literal=two words".into()],
        };
        let startup = match Some(command) {
            Some(command) => WorkspaceStartup::LaunchTerminal {
                cwd: Some(PathBuf::from("C:/work area")),
                command,
            },
            None => unreachable!(),
        };
        let WorkspaceStartup::LaunchTerminal { cwd, command } = startup else {
            panic!("explicit launch was discarded");
        };
        assert_eq!(cwd, Some(PathBuf::from("C:/work area")));
        assert_eq!(command.program(), "shell.exe");
        assert_eq!(command.args(), &["--literal=two words".to_owned()]);
    }
}
