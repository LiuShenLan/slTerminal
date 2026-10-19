//! 复制动作的短时限视觉反馈(§5.2 合同)。
//!
//! 「复制 → 对勾」的小幅就地过渡态。刻意随宿主 GPUI 实体存放,不进全局
//! toast 或应用单例:宿主 Drop 则定时器一并 Drop。
//!
//! 与上游的形态差:上游 `CopyFeedback` 自成实体(方法收 `Context<'_, Self>`);
//! 本仓它是 `TerminalView` 的平铺字段(字段集),故方法泛化到宿主
//! 实体上下文,经 `slot` 访问器回指自身——测试仍以 CopyFeedback 自成实体
//! 运行(`slot = |s| s`),行为与上游逐字一致。

use std::time::Duration;

use gpui::{Context, Task};

/// 成功复制态的停留时长。
pub(crate) const COPY_FEEDBACK_TTL: Duration = Duration::from_millis(1500);

/// 单控件的复制确认状态。
///
/// 剪贴板 API 目前只给 best-effort 的 `()` 回报:调用方只能在已产出有效
/// 剪贴板载荷后调 `mark_copied`;出错/不可用路径必须保持原状。
#[derive(Default)]
pub(crate) struct CopyFeedback {
    copied: bool,
    pending: bool,
    generation: u64,
    expiry: Option<Task<()>>,
}

impl CopyFeedback {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)] // 复制反馈展示消费点在键位接线后启用。
    pub(crate) fn is_copied(&self) -> bool {
        self.copied
    }

    #[allow(dead_code)] // 异步剪贴板载荷路径随粘贴确认链点亮。
    pub(crate) fn is_pending(&self) -> bool {
        self.pending
    }

    /// 异步剪贴板载荷制备期间保持动作可见。开始新操作同时取消本控件
    /// 持有的旧成功过期定时器。
    #[allow(dead_code)] // 同上:只有同步复制路径(mark_copied)。
    pub(crate) fn mark_pending<E: 'static>(
        &mut self,
        cx: &mut Context<'_, E>,
        _slot: impl Fn(&mut E) -> &mut Self + Send + 'static,
    ) {
        self.expiry = None;
        self.generation = self.generation.wrapping_add(1);
        self.pending = true;
        self.copied = false;
        cx.notify();
    }

    /// 进入对勾态并安排有界自动复位。
    ///
    /// 代际守卫防止用户连点时旧定时器清掉新确认;替换 `expiry` 也会 Drop
    /// 上一个任务,连点不累积定时器。
    pub(crate) fn mark_copied<E: 'static>(
        &mut self,
        cx: &mut Context<'_, E>,
        slot: impl Fn(&mut E) -> &mut Self + Send + 'static,
    ) {
        self.expiry = None;
        self.pending = false;
        self.copied = true;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.expiry = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPY_FEEDBACK_TTL).await;
            let _ = this.update(cx, |host, cx| {
                let feedback = slot(host);
                if feedback.generation == generation {
                    feedback.copied = false;
                    feedback.expiry = None;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    /// 离开 pending/成功态,不呈现假成功。
    #[allow(dead_code)] // 失败路径消费点随复制合同用例点亮。
    pub(crate) fn clear<E: 'static>(
        &mut self,
        cx: &mut Context<'_, E>,
        _slot: impl Fn(&mut E) -> &mut Self + Send + 'static,
    ) {
        self.expiry = None;
        self.generation = self.generation.wrapping_add(1);
        self.pending = false;
        self.copied = false;
        cx.notify();
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod copy_feedback_tests {
    use super::*;
    use gpui::{AppContext as _, TestAppContext};

    #[gpui::test]
    fn copy_feedback_uses_controlled_clock_for_success_expiry(cx: &mut TestAppContext) {
        let feedback = cx.update(|cx| cx.new(|_| CopyFeedback::new()));
        cx.update(|cx| {
            feedback.update(cx, |feedback, cx| feedback.mark_copied(cx, |s| s));
        });
        assert!(cx.read(|cx| feedback.read(cx).is_copied()));

        cx.executor()
            .advance_clock(COPY_FEEDBACK_TTL - Duration::from_millis(1));
        cx.run_until_parked();
        assert!(cx.read(|cx| feedback.read(cx).is_copied()));

        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert!(!cx.read(|cx| feedback.read(cx).is_copied()));
    }

    #[gpui::test]
    fn starting_async_copy_clears_old_success_state(cx: &mut TestAppContext) {
        let feedback = cx.update(|cx| cx.new(|_| CopyFeedback::new()));
        cx.update(|cx| {
            feedback.update(cx, |feedback, cx| feedback.mark_copied(cx, |s| s));
            feedback.update(cx, |feedback, cx| feedback.mark_pending(cx, |s| s));
        });
        assert!(cx.read(|cx| feedback.read(cx).is_pending()));
        assert!(!cx.read(|cx| feedback.read(cx).is_copied()));
    }
}
