//! 只读文本快照：worker 输出 revision 与当前画面＋回滚纯文本。

use crate::grid::Dimensions as _;
use crate::index::{Column, Point};

use super::Term;

/// 不可变文本快照：worker 输出 revision 与当前画面＋回滚的纯文本。
///
/// 只由 worker 在锁内捕获并经 reply 通道交给后台读取方；revision 取最近
/// 发布的帧代次，供分页游标判定旧页是否失效。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSnapshot {
    pub revision: u64,
    pub text: String,
}

impl<T> Term<T> {
    /// 只读导出当前画面与回滚的纯文本；供 Runtime 分页读取，不修改网格。
    ///
    /// 行间以 `\n` 连接，自动换行行按网格拼接，宽字符按 cell 原样保留；
    /// 末尾换行被收束，空行保留为独立的空行。
    pub fn scrollback_text(&self) -> String {
        self.bounds_to_string(
            Point::new(self.topmost_line(), Column(0)),
            Point::new(self.bottommost_line(), self.last_column()),
        )
    }
}
