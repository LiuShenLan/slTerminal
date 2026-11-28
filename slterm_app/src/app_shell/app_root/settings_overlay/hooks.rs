//! SettingsView 的 Hooks 草稿、编辑模式与诊断瞬态通道。

#[cfg(all(test, feature = "gpui-test-support"))]
use gpui::Focusable as _;
use gpui::{Context, Entity, Window};
use gpui_component::input::InputState;

use super::SettingsView;

impl SettingsView {
    /// Hooks JSON 编辑只写内存草稿；非法文本保留最后合法模型。
    pub(in crate::app_shell::app_root) fn on_hooks_text_change(
        &mut self,
        input: Entity<InputState>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(key) = self.hooks_sync_key else {
            return;
        };
        let text = input.read(cx).value().to_string();
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |root, cx| root.update_hooks_draft_text(key, &text, cx));
        }
    }

    pub(in crate::app_shell::app_root) fn bind_panel_id(
        &mut self,
        panel_id: slterm_workspace::PanelId,
        cx: &mut Context<'_, Self>,
    ) {
        if self.panel_id == Some(panel_id) {
            return;
        }
        self.panel_id = Some(panel_id);
        cx.notify();
    }

    pub(in crate::app_shell::app_root) const fn panel_id_value(
        &self,
    ) -> Option<slterm_workspace::PanelId> {
        self.panel_id
    }

    pub(in crate::app_shell::app_root) const fn hooks_layer_value(
        &self,
    ) -> crate::ai_hook::draft_host::HooksDraftLayer {
        self.hooks_layer
    }

    pub(in crate::app_shell::app_root) fn set_hooks_layer_value(
        &mut self,
        layer: crate::ai_hook::draft_host::HooksDraftLayer,
    ) {
        self.hooks_layer = layer;
    }

    pub(in crate::app_shell::app_root) fn reset_hooks_sync(&mut self) {
        self.hooks_sync_key = None;
    }

    pub(in crate::app_shell::app_root) const fn hooks_mode_value(
        &self,
    ) -> super::super::hooks_page::HooksEditMode {
        self.hooks_mode
    }

    /// 切换 JSON／GUI：重置两端的帧间同步，草稿文本与模型不变。
    pub(in crate::app_shell::app_root) fn set_hooks_mode(
        &mut self,
        mode: super::super::hooks_page::HooksEditMode,
        cx: &mut Context<'_, Self>,
    ) {
        if self.hooks_mode == mode {
            return;
        }
        self.hooks_mode = mode;
        self.hooks_sync_key = None;
        self.hooks_form.reset_sync();
        self.hooks_diagnostics_expanded = false;
        cx.notify();
    }

    pub(in crate::app_shell::app_root) const fn hooks_diagnostics_expanded(&self) -> bool {
        self.hooks_diagnostics_expanded
    }

    pub(in crate::app_shell::app_root) fn toggle_hooks_diagnostics(
        &mut self,
        cx: &mut Context<'_, Self>,
    ) {
        self.hooks_diagnostics_expanded = !self.hooks_diagnostics_expanded;
        cx.notify();
    }

    pub(in crate::app_shell::app_root) fn reset_hooks_diagnostics(&mut self) {
        self.hooks_diagnostics_expanded = false;
    }

    pub(in crate::app_shell::app_root) fn hooks_form_mut(
        &mut self,
    ) -> &mut super::super::hooks_page::HooksFormState {
        &mut self.hooks_form
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(in crate::app_shell::app_root) fn hooks_form_ref(
        &self,
    ) -> &super::super::hooks_page::HooksFormState {
        &self.hooks_form
    }

    pub(in crate::app_shell::app_root) fn hooks_input(&self) -> Option<&Entity<InputState>> {
        self.hooks_input.as_ref()
    }

    /// 测试把真实键盘焦点交给 Hooks JSON 编辑器。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(in crate::app_shell::app_root) fn focus_hooks_input_for_test(
        &self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(input) = &self.hooks_input {
            let handle = input.read(cx).focus_handle(cx);
            handle.focus(window, cx);
        }
    }
}
