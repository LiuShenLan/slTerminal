//! 窗口几何观察与按机器去抖保存。
//!
//! 前台只记录最新几何；写盘经工作区 writer 的封闭命令，连续拖动不放大写。

use std::time::Duration;

use gpui::{Context, Window};

use super::AppRoot;
use crate::windowing::geometry::WindowGeometry;

/// 窗口拖动与缩放的去抖窗口；与既有工作区状态的去抖节奏一致。
const WINDOW_GEOMETRY_DEBOUNCE: Duration = Duration::from_millis(500);

impl AppRoot {
    /// 观察窗口边界变化；注册即记录一次当前几何（恢复夹回后的真实值）。
    pub(crate) fn observe_window_geometry(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let subscription = cx.observe_window_bounds(window, |root, window, cx| {
            root.record_window_geometry(window, cx);
        });
        self.window_geometry_subscription = Some(subscription);
    }

    fn record_window_geometry(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let geometry = WindowGeometry::from_window(
            window.window_bounds(),
            window.display(cx).map(|display| display.visible_bounds()),
        );
        if self.window_geometry == Some(geometry) {
            return;
        }
        self.window_geometry = Some(geometry);
        let Some(writer) = self.project_writer.clone() else {
            // 无 writer 的测试/内存根只保留观察值，不伪造持久事实。
            return;
        };
        // 连续变化只保留最后一次：替换任务即取消上一次去抖。
        self.window_geometry_save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(WINDOW_GEOMETRY_DEBOUNCE)
                .await;
            let _ = this.update(cx, |root, _| {
                let Some(geometry) = root.window_geometry else {
                    return;
                };
                if let Err(error) = writer.save_window_geometry(&root.machine_key, &geometry) {
                    log::warn!("窗口几何写队列不可用: {error}");
                }
            });
        }));
    }
}
