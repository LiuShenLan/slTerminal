//! 更新安装的关闭守卫接线：预约只在统一关闭完成后保留。
//!
//! 更新提示 UI 的后台准备完成后经本模块入口排定安装；关闭被取消、失败
//! 或窗口销毁时守卫写回撤销，下一次启动不安装。

#[cfg(all(test, feature = "gpui-test-support"))]
use std::sync::Arc;

use gpui::{Context, Window};

use super::AppRoot;
use super::commands::AppRootError;
use crate::update_check::UpdateAsset;
use crate::update_download::handoff::UpdateInstallGuard;

/// 测试注入的预约端口；生产恒走真实分发形态与安装包就绪检查。
#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) type UpdateInstallArmHook =
    Arc<dyn Fn(&UpdateAsset) -> Result<UpdateInstallGuard, String>>;

impl AppRoot {
    /// 预约安装守卫；测试端口只替换持久文件根，关闭链仍走同一入口。
    fn arm_update_install(&self, asset: &UpdateAsset) -> Result<UpdateInstallGuard, String> {
        #[cfg(all(test, feature = "gpui-test-support"))]
        if let Some(arm) = &self.update_install_arm_for_test {
            return arm(asset);
        }
        UpdateInstallGuard::arm(crate::platform::dirs::local_dir(), asset)
    }

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
            self.arm_update_install(&asset)
                .map_err(|_| AppRootError::UpdateInstall)?,
        );
        Ok(self.request_close(window, cx))
    }

    /// 测试：替换预约端口；文件根由用例注入，不读写真实数据目录。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_update_install_arm_for_test(&mut self, arm: UpdateInstallArmHook) {
        self.update_install_arm_for_test = Some(arm);
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
