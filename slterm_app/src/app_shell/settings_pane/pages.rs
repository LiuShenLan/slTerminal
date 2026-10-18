//! 各分区页:只摆控件,不定义设置语义。所有改动经统一写通道
//! (`SettingsPane::try_persist`)落盘并重载。

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, Window, div, px,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::select::Select;
use gpui_component::slider::Slider;
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme as _, h_flex, v_flex};

use super::SettingsPane;
use super::design::{SETTINGS_GROUP_GAP, SETTINGS_SELECT_WIDTH};

impl SettingsPane {
    pub(super) fn select_row(
        &self,
        key: &'static str,
        label: &'static str,
        desc: &'static str,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let control = match self.selects.iter().find(|entry| entry.key == key) {
            Some(entry) => div()
                .debug_selector(move || format!("settings-select-{key}"))
                .w(px(SETTINGS_SELECT_WIDTH))
                .child(
                    Select::new(&entry.select)
                        .appearance(false)
                        .h_full()
                        .rounded(px(6.0)),
                )
                .into_any_element(),
            None => div().into_any_element(),
        };
        self.row(label, desc, control, cx).into_any_element()
    }

    pub(super) fn switch_row(
        &self,
        key: &'static str,
        label: &'static str,
        desc: &'static str,
        checked: bool,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let control = div()
            .debug_selector(move || format!("settings-switch-{key}"))
            .child(Switch::new(key).checked(checked).on_click(cx.listener(
                move |this, checked: &bool, window, cx| {
                    this.toggle(key, *checked, window, cx);
                },
            )))
            .into_any_element();
        self.row(label, desc, control, cx).into_any_element()
    }

    pub(super) fn input_row(
        &self,
        key: &'static str,
        label: &'static str,
        desc: &'static str,
        input: &gpui::Entity<InputState>,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let state = input.clone();
        let control = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(
                div()
                    .debug_selector(move || format!("settings-input-{key}"))
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(input)),
            )
            .child(
                Button::new(SharedString::from(format!("settings-save-{key}")))
                    .label(self.language(cx).pick("保存", "Save"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let value = state.read(cx).value().to_string();
                        this.save_text_input(key, value, window, cx);
                    })),
            )
            .into_any_element();
        self.row(label, desc, control, cx).into_any_element()
    }

    pub(super) fn slider_row(
        &self,
        label: &'static str,
        desc: &'static str,
        debug_id: &'static str,
        slider: &gpui::Entity<gpui_component::slider::SliderState>,
        display: SharedString,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let control = h_flex()
            .w(px(SETTINGS_SELECT_WIDTH))
            .items_center()
            .gap_3()
            .child(
                div()
                    .debug_selector(move || debug_id.to_owned())
                    .flex_1()
                    .min_w_0()
                    .child(Slider::new(slider)),
            )
            .child(
                div()
                    .w(px(48.0))
                    .flex_shrink_0()
                    .text_color(cx.theme().muted_foreground)
                    .child(display),
            )
            .into_any_element();
        self.row(label, desc, control, cx).into_any_element()
    }

    pub(super) fn page_appearance(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(
                self.group(language.pick("字体与排版", "Fonts and typography"), cx)
                    .child(self.input_row(
                        "font_family",
                        language.pick("字体", "Font family"),
                        language.pick(
                            "逗号分隔主字体与回退字体；对新终端生效。",
                            "Comma-separated primary and fallback families; applies to new terminals.",
                        ),
                        &self.font_family_input,
                        cx,
                    ))
                    .child(self.input_row(
                        "font_size",
                        language.pick("终端字号", "Terminal font size"),
                        language.pick(
                            "逻辑像素，4–96；对新终端生效。",
                            "Logical pixels, 4–96; applies to new terminals.",
                        ),
                        &self.font_size_input,
                        cx,
                    ))
                    .child(self.select_row(
                        "ligatures",
                        language.pick("连字", "Ligatures"),
                        language.pick(
                            "跟随主题时由主题声明决定。",
                            "Follow theme defers to the theme typography.",
                        ),
                        cx,
                    ))
                    .child(self.select_row(
                        "cell_width_mode",
                        language.pick("字宽模式", "Cell width"),
                        "",
                        cx,
                    ))
                    .child(self.select_row(
                        "theme",
                        language.pick("主题", "Theme"),
                        language.pick(
                            "仅暗色内置主题；主题编辑归后续阶段。",
                            "Dark built-in themes only; the theme editor follows later.",
                        ),
                        cx,
                    )),
            )
            .child(
                self.group(language.pick("显示", "Display"), cx)
                    .child(self.slider_row(
                        language.pick("窗口不透明度", "Window opacity"),
                        language.pick(
                            "拖动即时预览，松手保存。",
                            "Drag to preview; release to save.",
                        ),
                        "settings-slider-opacity",
                        &self.opacity_slider,
                        SharedString::from(format!("{:.0}%", self.runtime.opacity * 100.0)),
                        cx,
                    ))
                    .child(self.slider_row(
                        language.pick("滚动速度", "Scroll speed"),
                        language.pick(
                            "拖动即时预览，松手保存；0.25×–4×。",
                            "Drag to preview; release to save; 0.25×–4×.",
                        ),
                        "settings-slider-scroll-speed",
                        &self.scroll_speed_slider,
                        SharedString::from(format!("{:.2}×", self.runtime.scroll_speed)),
                        cx,
                    ))
                    .child(self.select_row(
                        "scrollback_lines",
                        language.pick("回滚行数", "Scrollback lines"),
                        language.pick(
                            "只影响新建终端，不改已打开会话。",
                            "Applies to new terminals; open sessions keep their history.",
                        ),
                        cx,
                    ))
                    .child(self.select_row(
                        "density",
                        language.pick("密度", "Density"),
                        "",
                        cx,
                    ))
                    .child(self.select_row(
                        "tab_reveal",
                        language.pick("标签展开动效", "Tab reveal"),
                        "",
                        cx,
                    ))
                    .child(self.switch_row(
                        "tab_close_visible",
                        language.pick("显示标签关闭按钮", "Show tab close buttons"),
                        language.pick(
                            "关闭后仍可用中键关闭。",
                            "Middle-click still closes tabs when hidden.",
                        ),
                        self.runtime.tab_close_visible,
                        cx,
                    ))
                    .child(self.switch_row(
                        "cjk_bold_regular",
                        language.pick("中日韩粗体提亮", "Brighten CJK bold"),
                        "",
                        self.runtime.cjk_bold_regular,
                        cx,
                    )),
            )
    }

    pub(super) fn page_profiles(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        let startup_directory = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(
                div()
                    .debug_selector(|| "settings-input-startup_directory".to_owned())
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&self.startup_directory_input)),
            )
            .child(
                Button::new("settings-pick-startup-directory")
                    .label(language.pick("选择…", "Choose..."))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_startup_directory(window, cx);
                    })),
            )
            .into_any_element();
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(
                self.group(language.pick("启动", "Startup"), cx)
                    .child(self.row(
                        language.pick("启动目录", "Startup directory"),
                        language.pick(
                            "留空继承当前目录；对新终端生效。",
                            "Empty inherits the current directory; applies to new terminals.",
                        ),
                        startup_directory,
                        cx,
                    ))
                    .child(self.switch_row(
                        "refresh_environment",
                        language.pick("刷新环境变量", "Refresh environment"),
                        language.pick(
                            "新进程读取最新系统环境变量。",
                            "New processes read the latest system environment.",
                        ),
                        self.runtime.refresh_environment,
                        cx,
                    )),
            )
            .child(
                self.group(language.pick("补全", "Completion"), cx)
                    .child(self.switch_row(
                        "ghost",
                        language.pick("命令补全", "Command completion"),
                        "",
                        self.runtime.ghost,
                        cx,
                    ))
                    .child(self.select_row(
                        "completion_style",
                        language.pick("补全样式", "Completion style"),
                        "",
                        cx,
                    ))
                    .child(self.select_row(
                        "accept",
                        language.pick("接受补全键", "Accept completion key"),
                        "",
                        cx,
                    )),
            )
            .child(
                self.group(language.pick("提醒", "Alerts"), cx)
                    .child(self.switch_row(
                        "ai_toasts",
                        language.pick("AI 消息通知", "AI notifications"),
                        "",
                        self.runtime.ai_toasts,
                        cx,
                    ))
                    .child(self.select_row(
                        "notification_duration",
                        language.pick("通知时长", "Notification duration"),
                        language.pick(
                            "默认档不覆盖各消息自带时长。",
                            "Default keeps each notification's own duration.",
                        ),
                        cx,
                    ))
                    .child(self.select_row(
                        "bell",
                        language.pick("终端铃声", "Terminal bell"),
                        "",
                        cx,
                    )),
            )
    }

    pub(super) fn page_interaction(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(
                self.group(language.pick("鼠标与选区", "Mouse and selection"), cx)
                    .child(self.switch_row(
                        "focus_follows_mouse",
                        language.pick("焦点跟随鼠标", "Focus follows mouse"),
                        "",
                        self.runtime.focus_follows_mouse.unwrap_or(false),
                        cx,
                    ))
                    .child(self.switch_row(
                        "copy_on_select",
                        language.pick("选中即复制", "Copy on select"),
                        "",
                        self.runtime.copy_on_select,
                        cx,
                    ))
                    .child(self.switch_row(
                        "multiline_paste_confirm",
                        language.pick("粘贴前确认", "Confirm pastes"),
                        language.pick(
                            "换行/提权命令/控制字符先确认。",
                            "Confirms newlines, elevated commands and control characters.",
                        ),
                        self.runtime.multiline_paste_confirm,
                        cx,
                    ))
                    .child(self.switch_row(
                        "panel_resize",
                        language.pick("拖拽调节侧栏宽度", "Drag to resize the sidebar"),
                        "",
                        self.runtime.panel_resize,
                        cx,
                    )),
            )
            .child(
                self.group(language.pick("光标", "Cursor"), cx)
                    .child(self.select_row(
                        "cursor_shape",
                        language.pick("光标形状", "Cursor shape"),
                        "",
                        cx,
                    ))
                    .child(self.switch_row(
                        "cursor_blink",
                        language.pick("光标闪烁", "Cursor blink"),
                        "",
                        self.runtime.cursor_blink.unwrap_or(true),
                        cx,
                    ))
                    .child(self.select_row(
                        "cursor_motion",
                        language.pick("平滑光标", "Smooth cursor motion"),
                        "",
                        cx,
                    )),
            )
            .child(
                self.group(language.pick("标签与窗口", "Tabs and windows"), cx)
                    .child(self.select_row(
                        "new_tab_position",
                        language.pick("新标签位置", "New tab position"),
                        "",
                        cx,
                    ))
                    .child(self.select_row(
                        "vcs_display",
                        language.pick("侧栏版本控制", "Sidebar version control"),
                        "",
                        cx,
                    ))
                    .child(self.switch_row(
                        "dim_inactive_panes",
                        language.pick("压暗非聚焦分屏", "Dim inactive panes"),
                        "",
                        self.runtime.dim_inactive_panes,
                        cx,
                    )),
            )
    }

    pub(super) fn page_advanced(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::Div {
        let language = self.language(cx);
        v_flex()
            .w_full()
            .gap(px(SETTINGS_GROUP_GAP))
            .child(
                self.group(language.pick("会话生命周期", "Session lifecycle"), cx)
                    .child(self.switch_row(
                        "restore_session",
                        language.pick("启动时恢复上次标签", "Restore previous tabs at startup"),
                        "",
                        self.runtime.restore_session,
                        cx,
                    ))
                    .child(self.switch_row(
                        "resume_ai",
                        language.pick("恢复时接续 AI 对话", "Resume AI conversations"),
                        "",
                        self.runtime.resume_ai,
                        cx,
                    ))
                    .child(self.switch_row(
                        "keep_session",
                        language.pick("关窗后保留后台会话", "Keep sessions after window close"),
                        "",
                        self.runtime.keep_session,
                        cx,
                    ))
                    .child(self.switch_row(
                        "silent_start",
                        language.pick("静默启动", "Silent start"),
                        "",
                        self.runtime.silent_start,
                        cx,
                    ))
                    .child(self.switch_row(
                        "launch_at_login",
                        language.pick("开机启动", "Launch at login"),
                        "",
                        self.runtime.launch_at_login,
                        cx,
                    ))
                    .child(self.switch_row(
                        "tray",
                        language.pick("常驻系统托盘", "System tray icon"),
                        "",
                        self.runtime.tray,
                        cx,
                    )),
            )
            .child(
                self.group(language.pick("更新", "Updates"), cx)
                    .child(self.switch_row(
                        "auto_check_updates",
                        language.pick("自动检查更新", "Check for updates"),
                        "",
                        self.runtime.auto_check_updates,
                        cx,
                    ))
                    .child(self.switch_row(
                        "auto_download_updates",
                        language.pick("后台下载更新", "Download updates in background"),
                        language.pick("不授予安装权限。", "Does not grant install permission."),
                        self.runtime.auto_download_updates,
                        cx,
                    ))
                    .child(self.switch_row(
                        "fetch",
                        language.pick("新会话欢迎屏", "Welcome screen"),
                        "",
                        self.runtime.fetch,
                        cx,
                    )),
            )
            .child(
                self.group(language.pick("环境", "Environment"), cx)
                    .child(self.switch_row(
                        "terminal_proxy",
                        language.pick("终端使用系统代理", "Use system proxy"),
                        language.pick(
                            "新建本地终端写入代理环境变量。",
                            "New local terminals inherit the system proxy environment.",
                        ),
                        self.runtime.terminal_proxy,
                        cx,
                    )),
            )
    }
}
