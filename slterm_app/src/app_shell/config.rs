//! 壳侧全局 `Settings`(M3 最小消费面,13 篇锚点;06 篇 `RuntimeSettings` 的
//! GUI 化消费白名单即下列字段,M6 换 JSON 读通道时字段名/默认值零漂移)。

use gpui::Global;

use crate::gpui_terminal::colors::Palette;

/// 默认字号:11.25pt × 4/3 = 15px(与上游配置缺省同链)。
const DEFAULT_FONT_SIZE_PX: f32 = 15.0;
/// 滚轮速度缺省(上游 settings `DEFAULT_SCROLL_SPEED` 同值)。
const DEFAULT_SCROLL_SPEED: f32 = 1.0;
/// 回滚行数(D02-3 裁决值)。
const SCROLLING_HISTORY: usize = 10000;

/// 应用启动时装载一次的全局设置;M3 无热应用(apply_runtime_settings 归 06)。
pub struct Settings {
    pub font_family: String,
    pub font_bold_family: String,
    pub font_italic_family: String,
    pub font_bold_italic_family: String,
    /// GPUI 逻辑像素(1pt = 4/3 px @96dpi)。
    pub font_size_px: f32,
    pub ligatures: bool,
    /// 暗色单份(仅暗色定位;D06-3)。
    pub palette: Palette,
    /// 选区完成即复制;M3 默认关(06 篇设置键回填)。
    pub copy_on_select: bool,
    pub scroll_speed: f32,
}

impl Global for Settings {}

impl Settings {
    /// M3 = 常量默认;设置 JSON 读通道归 06 篇。
    pub fn load() -> Self {
        // 四族同 defaulted 到内嵌 Maple:系统等宽字体无 NF 图标码点,
        // 内嵌字形同源是跨机器可比对的前提(assets.rs 注册链)。
        let family = crate::app_shell::assets::REQUIRED_FONT_FAMILY.to_owned();
        Settings {
            font_family: family.clone(),
            font_bold_family: family.clone(),
            font_italic_family: family.clone(),
            font_bold_italic_family: family,
            font_size_px: DEFAULT_FONT_SIZE_PX,
            ligatures: true,
            palette: Palette::default(),
            copy_on_select: false,
            scroll_speed: DEFAULT_SCROLL_SPEED,
        }
    }

    /// 引擎 `Term` 的启动配置。
    ///
    /// ConPTY 关键位照上游钉死(因果链):侧载 ConPTY 的起桥 DA1 已被宿主
    /// 预应答,Term 必须吞掉自己的重复应答(`suppress_bringup_da1`,上游 =
    /// `conpty_sideload_enabled()`——core 已无该读法,Options 默认侧载恒真,
    /// 此处硬编码 true);resize 后 ConPTY 静默重锚主屏,网格必须同语义
    /// (`conpty_resize`);`kitty_keyboard` 开增强键报。
    pub fn term_config(&self) -> slterm_terminal::term::Config {
        slterm_terminal::term::Config {
            scrolling_history: SCROLLING_HISTORY,
            suppress_bringup_da1: true,
            conpty_resize: true,
            kitty_keyboard: true,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn load_pins_the_m3_constant_defaults() {
        let settings = Settings::load();
        for family in [
            &settings.font_family,
            &settings.font_bold_family,
            &settings.font_italic_family,
            &settings.font_bold_italic_family,
        ] {
            assert_eq!(family, crate::app_shell::assets::REQUIRED_FONT_FAMILY);
        }
        assert_eq!(settings.font_size_px, 15.0);
        assert!(settings.ligatures);
        assert!(!settings.copy_on_select);
        assert_eq!(settings.scroll_speed, 1.0);
    }

    #[test]
    fn term_config_carries_the_anchored_scrollback() {
        // D02-3:回滚 10000 行是产品裁决值,不是库默认巧合——钉字面量。
        assert_eq!(Settings::load().term_config().scrolling_history, 10000);
    }

    #[test]
    fn term_config_pins_the_conpty_and_keyboard_flags() {
        // 上游同链三键:侧载 DA1 预应答 / ConPTY 网格同语义 / kitty 增强键报。
        let config = Settings::load().term_config();
        assert!(config.suppress_bringup_da1);
        assert!(config.conpty_resize);
        assert!(config.kitty_keyboard);
    }
}
