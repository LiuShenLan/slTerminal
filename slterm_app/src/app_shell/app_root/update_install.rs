//! 更新安装的关闭守卫接线：预约只在统一关闭完成后保留。
//!
//! 更新提示 UI 的后台准备完成后经本模块入口排定安装；关闭被取消、失败
//! 或窗口销毁时守卫写回撤销，下一次启动不安装。

use gpui::{Context, Window};

use super::AppRoot;
use super::commands::AppRootError;
use crate::update_check::UpdateAsset;
use crate::update_download::handoff::UpdateInstallGuard;

impl AppRoot {
    /// 更新提示 UI 的后台回执入口：预约安装并启动统一关闭。
    ///
    /// 返回 `true` 表示关闭已开始推进；`false` 表示关闭未开始（例如用户
    /// 取消），守卫保持待定，用户显式放弃时调用 [`Self::cancel_update_install`]。
    #[allow(dead_code)] // 消费点 = 更新提示 UI 的后台准备回执接线。
    pub(crate) fn begin_update_install(
        &mut self,
        asset: UpdateAsset,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<bool, AppRootError> {
        self.ensure_not_closing()?;
        if self.update_install.is_some() {
            return Err(AppRootError::UpdateInstall);
        }
        self.update_install = Some(
            UpdateInstallGuard::arm(crate::platform::dirs::local_dir(), &asset)
                .map_err(|_| AppRootError::UpdateInstall)?,
        );
        Ok(self.request_close(window, cx))
    }

    /// 用户显式放弃安装：守卫释放并撤销预约。
    #[allow(dead_code)] // 消费点 = 更新提示 UI 的取消动作。
    pub(crate) fn cancel_update_install(&mut self) {
        self.update_install = None;
    }

    /// 统一关闭 Completed：保留预约，下次启动执行安装。
    pub(super) fn finish_update_install(&mut self) {
        if let Some(guard) = self.update_install.take() {
            guard.install_scheduled();
        }
    }
}
