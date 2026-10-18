//! console 子系统对照目标(整文件技巧随上游随迁;开放
//! 问题 2,2026-10-09 裁决「保留重设」——重设为 console 形态启动行为的
//! 独立回归锚,防启动链回归)。
//!
//! 因果链:console 子系统目标自动继承父进程 Ctrl+C 状态;GUI 目标
//! AttachConsole 会重置该状态并掩盖 bug。本体(D04-1)同为 console
//! 子系统,继承语义由本体直接承载,但本体是产品进程、不自跑测试——
//! 本对照目标与本体同子系统形态,作继承语义的独立自动化回归锚。
//!
//! 运行纪律:用例 spawn 子进程发真 Ctrl+C,必须串行——
//! `cargo test -p slterm --test windows_console_startup -- --test-threads=1`。
#![cfg(windows)]
#![windows_subsystem = "console"] // 测试目标显式 console 子系统

#[path = "../src/platform/startup/console.rs"] // 挂载被测源码
mod console;
// 薄适配:console_tests 内经 `super::` 解析 `prepare_console_for_gui`——
// 上游 lib 树内其 super = console 模块;本 target 其 super = crate root,
// 经此 re-export 对齐(上游同形态)。
#[allow(unused_imports)]
use console::prepare_console_for_gui;

#[path = "../src/platform/startup/console_tests.rs"] // 挂载其测试模块
mod console_tests;
