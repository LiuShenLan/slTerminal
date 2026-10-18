//! 品牌常量表(`slterm_app::brand`,归属权登记于类型锚点表,
//! 各领域只消费不重复定义)。

/// 产品名:窗口标题、panic 弹面标题、CLI 帮助(归 04)与安装器(归 12)共用。
pub const NAME: &str = "slTerminal";

/// Windows AppUserModelID(P-3):toast 归因与任务栏归组归通知与任务栏域消费,
/// 开窗 `app_id` 由 app_shell 直接使用。
pub const WINDOWS_APP_ID: &str = "com.slterminal.terminal";

/// 产品一句话描述。
pub const DESCRIPTION: &str = "slTerminal — GPU-accelerated terminal for AI CLIs";

#[cfg(test)]
mod brand_tests {
    /// 值即契约(AUMID 一旦发布不可漂移),钉字面量防手滑。
    #[test]
    fn constants_pin_the_anchored_values() {
        assert_eq!(super::NAME, "slTerminal");
        assert_eq!(super::WINDOWS_APP_ID, "com.slterminal.terminal");
        assert_eq!(
            super::DESCRIPTION,
            "slTerminal — GPU-accelerated terminal for AI CLIs"
        );
    }
}
