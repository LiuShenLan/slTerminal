//! 会话事件泵:把终端事件公平送达所属视图。
//!
//! 上游 SSH 阶段泵随 `TerminalLaunch::Ssh` 消亡一并裁剪(按理想终态重设计时另立)。
use super::registry::TerminalTarget;
use super::view::TerminalView;
use futures::StreamExt as _;
use gpui::Context;

pub(super) fn attach(
    target: TerminalTarget,
    mut rx: super::event_mailbox::EventReceiver,
    cx: &mut Context<'_, TerminalView>,
) -> gpui::Task<()> {
    cx.spawn(async move |this, cx| {
        while let Some(event) = rx.next().await {
            // 合并同一批到达的事件,避免每个 Wakeup 都独立触发一帧。
            let mut batch = vec![event];
            while batch.len() < 128 {
                match rx.try_recv() {
                    Ok(event) => batch.push(event),
                    _ => break,
                }
            }
            let full_batch = batch.len() == 128;
            if this
                .update(cx, |view: &mut TerminalView, cx| {
                    for event in batch {
                        if view
                            .binding
                            .as_ref()
                            .is_some_and(|binding| binding.target() == target)
                        {
                            view.process_event(event, cx);
                        }
                    }
                })
                .is_err()
            {
                break;
            }
            // 满批说明下游还有积压:让出 1ms 给渲染,防输出洪流饿死 UI。
            if full_batch {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1))
                    .await;
            }
        }
    })
}
