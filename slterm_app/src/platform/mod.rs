//! 平台适配层(Windows-only;unix/macOS 臂随定位约束消亡,不撒 cfg)。
//!
//! 业务 `#[cfg(windows)]` 仅允许出现在 slterm_terminal 的 pty 模块(硬约束 9);
//! 本层是 Windows 原生代码的直接落点,不再带平台分支。

pub mod dirs;
// 安装渠道门禁(09 篇 M10.5):外部托管渠道不参与自更新。
pub(crate) mod distribution;
pub(crate) mod elevation;
// 系统文件操作(07 篇 M8.4):回收站删除 + 系统处理器打开。
pub(crate) mod file_operations;
pub(crate) mod keyboard;
// 补全路径拼写(08 篇 M9.1):大小写语义与回写分隔符;shell 引用仍归
// slterm_completions 的 command_context。
pub(crate) mod local_paths;
// 原生通知(AUMID 注册 + WinRT toast MTA worker,09 篇 M10.1)。
pub(crate) mod notifications;
// 系统 locale 探测源文件(src/platform/locale.rs)经 `i18n::locale` 的
// `#[path]` 复编进查询核心;此处不再二次挂模块(独立合同 workspace 复编
// 同一份实现,不引入应用壳)。
pub(crate) mod process;
// 有界可取消子进程输出(08 篇 M9.1):Git 发现的 3s/1MB 预算原语。
#[allow(dead_code)] // 消费点 = completion::metadata 的 Git 读取(M9.1 引擎已接,呈现归 M9.5)。
pub(crate) mod process_output;
pub(crate) mod process_snapshot;
pub(crate) mod startup;
// 安装器 handoff 的原生面(09 篇 M10.5):路径规范化/进程身份/helper 启动。
pub(crate) mod update_installation;
