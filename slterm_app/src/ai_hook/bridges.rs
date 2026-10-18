//! 插件落盘桥:opencode/pi 的事件转发载荷随编译期内嵌,运行时由 local
//! 安装器写进各家全局插件目录(remote SSH 臂整支裁剪,载荷只有本地形态)。
//!
//! 载荷内容与 opencode SDK / Pi 扩展 API 的演进解耦:它们只 shell out 到
//! `SLTERM_HOOK_EXE`,事件归一化全在脚本内完成。

pub(super) const OPENCODE_PLUGIN_JS: &str = include_str!("../../res/hooks/opencode.js");
pub(super) const PI_EXTENSION_TS: &str = include_str!("../../res/hooks/pi.ts");
