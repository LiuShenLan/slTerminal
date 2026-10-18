# slterm_app（application 层）

## 存在理由与边界

本 crate = GPUI 壳与一切应用编排：窗口/工作区、设置 UI 与写通道、Runtime API、AI CLI 集成、文件与编辑器、平台适配、i18n 与打包入口。core 能力（VT/PTY、设置值模型、分屏几何、补全匹配）只消费，不在本 crate 复刻。

- **不做第二执行体**：git、文件树、补全、AI 会话等每个领域只有一个执行/解析入口；面板与后台任务都消费同一入口。
- **壳即前端**：GPUI 前台直接持有实体与展示态，无跨进程投影层；状态变更后显式通知/重绘。
- **前台纪律**：阻塞 I/O、网络、子进程等待不得在 GPUI 前台线程执行；走 background executor 或专用线程，结果回传前台。

## 启动与生命周期

- 启动链单点在 `lib.rs` 的 `run()`；`main.rs` 只承载 Windows 子系统选择与委托，保持薄壳。
- 退出链单点：关窗/托盘退出/系统关机都走同一「终局快照 + 退出」链，脏文件确认不得被旁路。
- 进程级后台任务元数据单点在 `background_tasks`；新增任务 = 注册一条元数据 + 一端执行体，调度框架零改动。
- PTY 进程映射只在 `terminal_registry`；pane 视图只订阅会话切片。

## 写通道与安全

- 设置写盘单点 = `settings_store`（白名单、浅合并、原子写、`.bak`、损坏三态）；所有持久化经 `atomic_file`，不裸写。
- 安全审计只经 `audit` facade（target `audit`）；写包装外的设置写调用点由源码扫描用例拒绝。
- 凭据只经 `secrets`/凭据管理器门面；类型层不向 DTO/快照/日志暴露 token（SEC-18）。
- 加密备份白名单即安全边界：只读写列举的六类文件与主题目录，凭据值、AI 会话库、任意 home 路径永不进出包。

## 模块细节导航

设置页、工作区、文件编辑器、终端视图、数学渲染、平台集成、更新链等各自目录下的 CLAUDE.md 是该域的真值源；本文件只保留跨模块纪律。新增模块目录时同步建立其 CLAUDE.md。

## 测试

- 定向测试一律 `cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`（TQ-COV-06：禁 `--lib`/直打默认 lib target）。
- UI 用例 `#[cfg(all(test, feature = "gpui-test-support"))]` 双门控；feature 未开时整体不编译，生产构建不带测试夹具。
- 平台/网络/桌面会话依赖项登记 `.claude/test-exemptions.md`；不登记即违规。
