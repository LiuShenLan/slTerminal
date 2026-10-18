//! 壳侧全局 `Settings`(13 篇锚点):`RuntimeSettings` 的 GUI 消费视图。
//!
//! 值权威在 `slterm-settings`;本结构只做两件事:把宽容解析后的运行时
//! 设置折算成终端/壳直接消费的形态(字体四族、逻辑字号、调色板、滚速),
//! 以及承载 `Changed` 热应用后的全局事实源。设置页写入后经
//! [`Settings::from_runtime`] 重建并 `cx.set_global`——消费方始终读同一份。

use gpui::Global;

use crate::gpui_terminal::colors::Palette;
use slterm_settings::{RawSettings, RuntimeSettings};

/// 默认字号:11.25pt × 4/3 = 15px(与上游配置缺省同链)。
const DEFAULT_FONT_SIZE_PX: f32 = 15.0;
/// 回滚行数(D02-3 裁决值)。
const SCROLLING_HISTORY: usize = 10_000;

/// 应用运行期全局设置;设置页提交后整体替换并扇出到各终端视图。
#[derive(Clone)]
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
    /// 选区完成即复制。
    pub copy_on_select: bool,
    pub scroll_speed: f32,
}

impl Global for Settings {}

impl Default for Settings {
    fn default() -> Self {
        Self::from_runtime(&RuntimeSettings::from_raw(&RawSettings::default()))
    }
}

impl Settings {
    /// 生产入口:读 settings.json 并折算为消费视图。读取失败(权限/IO)
    /// 不把故障升级成启动失败——记录告警后回默认值,损坏三态的主文件
    /// quarantine 已在读通道内完成。
    pub fn load() -> Self {
        match crate::settings_store::load_settings() {
            Ok(result) => {
                if result.corrupted {
                    log::warn!("settings.json 损坏,已回退备份或默认值");
                }
                Self::from_runtime(&RuntimeSettings::from_raw(&result.data))
            }
            Err(error) => {
                log::warn!("读取设置失败,使用默认值: {error}");
                Self::default()
            }
        }
    }

    /// 宽容解析后的运行时设置 → 壳消费视图。字体四族共用同一字族链
    /// (粗体/斜体字族键不在本篇白名单);字号缺省 = 15px;连字 Theme 档
    /// 在主题 typography 落地前按「开」处理(与 `Ligatures::enabled(None)`
    /// 同语义)。
    pub fn from_runtime(runtime: &RuntimeSettings) -> Self {
        let family = runtime
            .font_family
            .as_deref()
            .map(str::trim)
            .filter(|family| !family.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| crate::app_shell::assets::REQUIRED_FONT_FAMILY.to_owned());
        Settings {
            font_family: family.clone(),
            font_bold_family: family.clone(),
            font_italic_family: family.clone(),
            font_bold_italic_family: family,
            font_size_px: runtime.font_size_px.unwrap_or(DEFAULT_FONT_SIZE_PX),
            ligatures: runtime.ligatures.enabled(None),
            palette: Palette::default(),
            copy_on_select: runtime.copy_on_select,
            scroll_speed: runtime.scroll_speed,
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

    fn runtime(json: &str) -> RuntimeSettings {
        RuntimeSettings::from_raw(&RawSettings::from_json_bytes(json.as_bytes()))
    }

    #[test]
    fn absent_settings_fall_back_to_the_m3_constant_defaults() {
        let settings = Settings::default();
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
    fn runtime_values_feed_the_shell_view() {
        let runtime = runtime(
            r#"{
                "appearance": {"font_family": "Cascadia Mono", "font_size": 18},
                "scrolling": {"scroll_speed": 2.5},
                "interaction": {"ligatures": "off", "copy_on_select": true}
            }"#,
        );
        let settings = Settings::from_runtime(&runtime);
        assert_eq!(settings.font_family, "Cascadia Mono");
        assert_eq!(settings.font_bold_family, "Cascadia Mono");
        assert_eq!(settings.font_size_px, 18.0);
        assert!(!settings.ligatures, "off 档必须关闭连字");
        assert!(settings.copy_on_select);
        assert_eq!(settings.scroll_speed, 2.5);
    }

    #[test]
    fn blank_font_family_keeps_the_bundled_fallback() {
        let runtime = runtime(r#"{"appearance": {"font_family": "   "}}"#);
        let settings = Settings::from_runtime(&runtime);
        assert_eq!(
            settings.font_family,
            crate::app_shell::assets::REQUIRED_FONT_FAMILY,
            "空白字族链必须回内嵌 Maple,不能留空导致方框"
        );
    }

    #[test]
    fn term_config_carries_the_anchored_scrollback() {
        // D02-3:回滚 10000 行是产品裁决值,不是库默认巧合——钉字面量。
        assert_eq!(Settings::default().term_config().scrolling_history, 10000);
    }

    #[test]
    fn term_config_pins_the_conpty_and_keyboard_flags() {
        // 上游同链三键:侧载 DA1 预应答 / ConPTY 网格同语义 / kitty 增强键报。
        let config = Settings::default().term_config();
        assert!(config.suppress_bringup_da1);
        assert!(config.conpty_resize);
        assert!(config.kitty_keyboard);
    }
}
