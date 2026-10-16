//! 广播输入的视图侧边界:把用户输入以**语义**上报给宿主。
//!
//! 合同全貌(13 篇):
//! - **发送方向**(`write_user_*`):先按本 pane 的模式编码并写 PTY,再 emit
//!   一条语义事件。emit 是无条件的,广播开关的判定在宿主(M5),视图这边
//!   不留第二份记账。
//! - **接收方向**(`apply_broadcast_*`):归 M5 随分屏广播落地——接收端按
//!   **自己**的 term mode 重新编码后直接写 PTY,不再 emit(无环保证)。
//!
//! M3.2 只有 `Text` 的 emit 路径(IME 提交/粘贴);`Key` 变体的生产者是
//! 键位编码器(`write_user_key`),随 M3.3 落地。

use gpui::Context;

use super::{TerminalView, TerminalViewEvent};

/// 一次用户输入的语义载荷。
#[derive(Clone)]
pub enum TerminalInput {
    /// 键位编码器路径(M3.3 落地生产点)。
    #[allow(dead_code)]
    Key(gpui::Keystroke),
    /// `paste` = 走粘贴语义。接收端据此决定要不要套 bracketed-paste 哨兵:
    /// 给单个字符套上哨兵会让 zsh/fish 拒绝执行。
    #[allow(dead_code)] // 载荷消费 = M5 广播扇出;M3 无接收端。
    Text { text: String, paste: bool },
}

impl TerminalView {
    /// IME 提交与粘贴:上报**文本**而不是已经加过哨兵的字节。
    pub(super) fn write_user_text(
        &mut self,
        text: String,
        paste: bool,
        bytes: Vec<u8>,
        cx: &mut Context<'_, Self>,
    ) {
        cx.emit(TerminalViewEvent::UserInput(TerminalInput::Text {
            text,
            paste,
        }));
        self.write_input(bytes, cx);
    }
}
