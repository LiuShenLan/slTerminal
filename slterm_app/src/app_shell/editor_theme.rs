//! 编辑器语义槽装配(07 篇 M8.5 改造节 8;06 篇缝合点 2)。
//!
//! 消费侧唯一装配点:编辑器/gutter/diff 只引用 [`EditorThemeTokens`] 字段,
//! 禁裸颜色字面量。槽值单源在 06 篇 `ReviewedPalette` / `ThemeUiColors`;
//! 本模块只做语义命名与派生(派生规则只在这里实现一次)。
//!
//! 装配时机 = `Settings::from_runtime`(启动读与设置页热应用同一路径);
//! 主题变更重建 tokens → 编辑器三消费方随全局 Settings 重渲。

use slterm_settings::{ReviewedPalette, Rgb8, ThemeUiColors};

/// 编辑器三消费方(源码态/gutter/diff)共用的语义颜色槽。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorThemeTokens {
    /// 编辑器正文底色(选区 alpha 合成,照抄 `ReviewedPalette::code_background`)。
    pub background: Rgb8,
    pub foreground: Rgb8,
    /// 行号/次要 chrome。
    pub muted: Rgb8,
    /// 光标行/选中边。
    pub accent: Rgb8,
    /// gutter 六值:未跟踪/新增/修改/删除/重命名/冲突。
    pub gutter_untracked: Rgb8,
    pub gutter_added: Rgb8,
    pub gutter_modified: Rgb8,
    pub gutter_deleted: Rgb8,
    pub gutter_renamed: Rgb8,
    pub gutter_conflicted: Rgb8,
    /// diff 行底色(语义色向背景低权重混合;derive 不合成第二色带)。
    pub diff_added_bg: Rgb8,
    pub diff_removed_bg: Rgb8,
    /// 预览链接 = accent 或语义 blue 槽。
    pub link: Rgb8,
}

/// 语义槽装配:绿色 = success、黄色 = warning、红色 = error、
/// 蓝色 = info、紫色 = 冲突、灰色 = 未跟踪。
pub fn editor_theme_tokens(palette: ReviewedPalette, ui: &ThemeUiColors) -> EditorThemeTokens {
    EditorThemeTokens {
        // 选中 alpha 合成照抄 `ReviewedPalette::code_background`,但取 UI 槽
        // 的 background/selection(custom theme 的 UI 值才正确;内置主题两
        // 者逐字一致)。
        background: composite_selection(ui.background, ui.selection),
        foreground: ui.foreground,
        muted: ui.muted,
        accent: ui.accent,
        gutter_untracked: ui.muted,
        gutter_added: ui.success,
        gutter_modified: ui.warning,
        gutter_deleted: ui.error,
        gutter_renamed: ui.info,
        gutter_conflicted: palette.purple,
        diff_added_bg: mix_rgb(ui.background, ui.success, 0.18),
        diff_removed_bg: mix_rgb(ui.background, ui.error, 0.18),
        link: ui.info,
    }
}

/// 选中色按 alpha 叠到背景(CSS 式合成;与 `ReviewedPalette::code_background`
/// 同一算法,输入换成 UI 槽)。
fn composite_selection(background: Rgb8, selection: [u8; 4]) -> Rgb8 {
    let alpha = selection[3] as u32;
    let mut out = [0; 3];
    for index in 0..3 {
        out[index] =
            ((selection[index] as u32 * alpha + background[index] as u32 * (255 - alpha) + 127)
                / 255) as u8;
    }
    out
}

/// 低权重派生:前景语义色向背景混合(与 06 篇选区合成同一 CSS 式算法)。
fn mix_rgb(background: Rgb8, foreground: Rgb8, weight: f32) -> Rgb8 {
    let weight = weight.clamp(0.0, 1.0);
    let mut out = [0; 3];
    for index in 0..3 {
        out[index] = (foreground[index] as f32 * weight + background[index] as f32 * (1.0 - weight))
            .round() as u8;
    }
    out
}

#[cfg(test)]
mod editor_theme_tests {
    use super::*;
    use slterm_settings::ThemeName;

    fn tokens() -> EditorThemeTokens {
        let palette = ThemeName::Linear.reviewed_palette();
        editor_theme_tokens(palette, &ThemeUiColors::from_palette(palette))
    }

    /// 语义槽单源:六色与 diff 底色来自 UI 槽,不引入独立色表。
    #[test]
    fn semantic_slots_follow_the_ui_tokens() {
        let palette = ThemeName::Linear.reviewed_palette();
        let ui = ThemeUiColors::from_palette(palette);
        let tokens = editor_theme_tokens(palette, &ui);
        assert_eq!(tokens.background, palette.code_background());
        assert_eq!(tokens.foreground, ui.foreground);
        assert_eq!(tokens.muted, ui.muted);
        assert_eq!(tokens.accent, ui.accent);
        assert_eq!(tokens.gutter_added, ui.success);
        assert_eq!(tokens.gutter_modified, ui.warning);
        assert_eq!(tokens.gutter_deleted, ui.error);
        assert_eq!(tokens.gutter_renamed, ui.info);
        assert_eq!(tokens.gutter_conflicted, palette.purple);
        assert_eq!(tokens.gutter_untracked, ui.muted);
        assert_eq!(tokens.link, ui.info);
    }

    /// diff 底色是背景与语义色的低权重混合:不等于任一原色,且明度接近背景。
    #[test]
    fn diff_backgrounds_are_derived_with_a_low_weight() {
        let tokens = tokens();
        assert_ne!(tokens.diff_added_bg, tokens.background);
        assert_ne!(tokens.diff_added_bg, tokens.gutter_added);
        assert_ne!(tokens.diff_removed_bg, tokens.background);
        assert_ne!(tokens.diff_removed_bg, tokens.gutter_deleted);
        assert_ne!(tokens.diff_added_bg, tokens.diff_removed_bg);
        for channel in 0..3 {
            let background = tokens.background[channel] as i32;
            let added = tokens.diff_added_bg[channel] as i32;
            assert!(
                (added - background).abs() <= 64,
                "混合权重要低,通道 {channel}: {background} vs {added}"
            );
        }
    }

    /// 六值 gutter 色两两不同(渲染语义可区分)。
    #[test]
    fn gutter_colors_are_distinct() {
        let tokens = tokens();
        let colors = [
            tokens.gutter_untracked,
            tokens.gutter_added,
            tokens.gutter_modified,
            tokens.gutter_deleted,
            tokens.gutter_renamed,
            tokens.gutter_conflicted,
        ];
        for left in 0..colors.len() {
            for right in left + 1..colors.len() {
                assert_ne!(colors[left], colors[right], "槽 {left} 与 {right} 撞色");
            }
        }
    }
}
