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

// 解析器不经本 crate 再导出（上游 `pub use vte` 不迁）：各模块直接
// `use vte::…` 引用，公共面只露 02 篇出口集合的领域类型。
