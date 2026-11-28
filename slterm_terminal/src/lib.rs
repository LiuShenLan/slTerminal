//! slTerminal 终端核心：网格、VT、PTY 行为。core 层，无本地边、不依赖渲染包。

#![warn(rust_2018_idioms, future_incompatible)]
#![deny(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]
#![cfg_attr(clippy, deny(warnings))]

pub mod event;
pub mod event_loop;
pub mod grid;
pub mod index;
pub mod osc_cwd;
pub mod render;
pub mod selection;
pub mod sync;
pub mod term;
pub mod thread;
pub mod tty;
pub mod vi_mode;

pub use crate::grid::Grid;
pub use crate::term::Term;

/// 跨 crate 共享的最小 ANSI 值类型锚点。
///
/// application 只经本模块命名终端 frame 暴露的第三方值类型，不得直接依赖
/// VT 解析器 crate。原因：类型同一性必须由 core 公开 API 保证，解析器升级
/// 只改本 crate。边界：新增跨 crate 值类型先登记到本模块，不扩散解析器内部类型。
pub mod ansi {
    pub use vte::ansi::{Color, CursorShape, CursorStyle, NamedColor, Rgb};
}

/// 依赖方测试驱动终端状态的最小入口；生产默认构建不暴露解析器。
#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    /// 用核心 VT 解析器消费一段原始 PTY 字节。
    pub fn advance<T: crate::event::EventListener>(term: &mut crate::Term<T>, bytes: &[u8]) {
        let mut parser: vte::ansi::Processor = vte::ansi::Processor::new();
        parser.advance(term, bytes);
    }
}

/// 进程级环境变量锁(仅测试):shell PATH 注入族与 conpty LOCALAPPDATA 提取族
/// 整体替换进程环境变量,并行执行互相污染——同一锁族内串行,其余用例不受影响。
/// 拿锁入口在两个 Guard 的构造器;毒化容错(panic 后测试继续串行)。
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// PTY-side startup profiling (`SLTERM_BOOT_TRACE=1`): times the ConPTY
/// bring-up stages the app-side boot trace cannot see — they happen inside
/// this crate and the console host (CreatePseudoConsole, shell attach, the
/// host's DA1 handshake window until the first conout bytes arrive).
pub(crate) fn pty_trace(label: &str) {
    use std::sync::OnceLock;
    use std::time::Instant;
    static T0: OnceLock<Instant> = OnceLock::new();
    static ON: OnceLock<bool> = OnceLock::new();
    let t0 = *T0.get_or_init(Instant::now);
    if *ON.get_or_init(|| std::env::var_os("SLTERM_BOOT_TRACE").is_some()) {
        eprintln!(
            "[pty  +{:>7.1}ms] {label}",
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
}

// 解析器实现不经本 crate 再导出（上游 `pub use vte` 不迁）：内部模块直接
// `use vte::…`；公共面只露领域出口类型与 `ansi` 模块登记的最小值类型锚点。
