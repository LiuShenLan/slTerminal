// PTY 模块：终端创建、读写、缩放、销毁 + shell 选择 + profile 注入 + Windows build 号

#[cfg(windows)]
pub mod conpty_api;
pub mod reader;
pub mod shell;
pub mod spawn;
pub mod win_build;
