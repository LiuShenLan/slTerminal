//! 窗口管理 facade:回窗与 HWND 查询的唯一落点。
//!
//! 壳层开窗(`app_shell::windowing`)只负责窗口定形;窗口聚焦/还原/句柄
//! 查询归本模块,避免 Win32 焦点原语散落到业务路径。

pub(crate) mod focus;
