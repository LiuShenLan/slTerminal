//! 未知编码打开失败的显式编码重开面。
//!
//! 自动识别只接受 BOM 或合法 UTF-8；编码歧义时由用户选定编码后复用同一
//! Document 读取链重开。本模块只持请求与焦点资源，不读盘、不建立面板。

use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, ParentElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_document::DocumentEncoding;

use super::AppRoot;
use super::commands::AppRootError;
use super::documents::DocumentOpenRequest;
use super::focus_manager::FocusRegistration;
use crate::i18n::Message;

/// 编码选择面：保存待重试的同一打开请求，选择后回到统一读取链。
pub(super) struct PendingEncodingPrompt {
    request: DocumentOpenRequest,
    focus_handle: FocusHandle,
    registration: FocusRegistration,
}

/// 可选编码按固定顺序展示；标签是协议名，不做语言分支。
const ENCODINGS: [(DocumentEncoding, &str, &str); 5] = [
    (DocumentEncoding::Utf8, "UTF-8", "encoding-prompt-utf8"),
    (
        DocumentEncoding::Utf16Le,
        "UTF-16 LE",
        "encoding-prompt-utf16le",
    ),
    (
        DocumentEncoding::Utf16Be,
        "UTF-16 BE",
        "encoding-prompt-utf16be",
    ),
    (DocumentEncoding::Gbk, "GBK", "encoding-prompt-gbk"),
    (
        DocumentEncoding::Gb18030,
        "GB18030",
        "encoding-prompt-gb18030",
    ),
];

impl AppRoot {
    /// 打开编码选择面；已有选择面只替换为最新失败请求，不叠加第二层模态。
    pub(super) fn open_encoding_prompt(
        &mut self,
        mut request: DocumentOpenRequest,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.cancel_encoding_prompt(window, cx)?;
        // 保存冲突优先于编码选择；两者不叠加。
        self.cancel_save_conflict(window, cx)?;
        request.prepare_for_encoding_prompt();
        let focus_handle = cx.focus_handle();
        let registration = self.focus_manager.register(
            &focus_handle,
            self.focus_manager.workspace_token(),
            None,
            true,
        )?;
        if let Err(error) = self.focus_manager.open_modal(
            self.focus_manager.workspace_token(),
            self.focus_manager.workspace_token(),
            registration.token(),
            window,
            cx,
        ) {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        self.encoding_prompt = Some(PendingEncodingPrompt {
            request,
            focus_handle,
            registration,
        });
        cx.notify();
        Ok(())
    }

    /// 取消：只收合选择面，不创建 Document 或面板。
    pub(super) fn cancel_encoding_prompt(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(host) = self.encoding_prompt.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(host.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 选定编码：复用同一请求重新进入 FIFO 读取与安装链。
    pub(super) fn resolve_encoding_prompt(
        &mut self,
        encoding: DocumentEncoding,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let Some(mut host) = self.encoding_prompt.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(host.registration.token());
        cx.notify();
        close?;
        unregister?;
        host.request.reopen_with_encoding(encoding);
        self.queue_document_open(host.request, None, window, cx);
        Ok(())
    }

    /// 模态选择面：遮罩拦截背景命中，Escape 等同取消。
    pub(super) fn render_encoding_prompt(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let host = self.encoding_prompt.as_ref()?;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let mut choices = div().flex().flex_col().gap_1();
        for (encoding, label, selector) in ENCODINGS {
            choices = choices.child(encoding_choice(
                selector,
                label,
                cx,
                cx.listener(move |root, _, window, cx| {
                    if let Err(error) = root.resolve_encoding_prompt(encoding, window, cx) {
                        log::warn!("以编码重新打开失败: {error:?}");
                    }
                }),
            ));
        }
        let dialog = div()
            .debug_selector(|| "encoding-prompt-dialog".to_owned())
            .track_focus(&host.focus_handle)
            .on_key_down(cx.listener(|root, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.cancel_encoding_prompt(window, cx)
                {
                    log::warn!("编码选择面取消失败: {error:?}");
                }
            }))
            .w(px(360.0))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .text_sm()
                    .child(language.text(Message::EditorEncodingOpenTitle)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::EditorEncodingOpenDetail)),
            )
            .child(choices)
            .child(div().flex().flex_row().justify_end().child(encoding_choice(
                "encoding-prompt-cancel",
                language.text(Message::EditorCancel),
                cx,
                cx.listener(|root, _, window, cx| {
                    if let Err(error) = root.cancel_encoding_prompt(window, cx) {
                        log::warn!("编码选择面取消失败: {error:?}");
                    }
                }),
            )));
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .bg(cx.theme().background.opacity(0.7))
                .flex()
                .items_center()
                .justify_center()
                .child(dialog)
                .into_any_element(),
        )
    }
}

fn encoding_choice(
    debug_id: &'static str,
    label: &'static str,
    cx: &Context<'_, AppRoot>,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(debug_id)
        .debug_selector(move || debug_id.to_owned())
        .px_3()
        .py_1()
        .border_1()
        .border_color(cx.theme().border)
        .text_sm()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
        .into_any_element()
}
