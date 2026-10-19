# Changelog

本文件随安装器与便携包内置；发布前必须完成对应版本小节。发布说明只写
有构建或真实运行证据的用户可感知变化，中英双语表达同一事实。

## 0.3.0 (未发布 / Unreleased)

### 新增 / Added

- Windows 原生 GPUI 单进程终端：单窗口单实例、仅暗色模式、GPU 加速渲染；
  不再依赖 WebView 或 Node 运行时。
- 标签页与分屏：终端标签页、网格分屏、窗格缩放、拖拽停靠；终端标签页
  的布局、启动身份与 WindowState 在重启后恢复。
- 设置中心：应用/外观/配置/交互/高级/本地化/重置全局页与 Agent 组；
  内置暗色主题（默认 Linear）；中英双语界面。
- 侧栏：活动栏、双槽侧栏骨架与文件浏览器（git 状态着色、创建/删除/重命名/
  打开、回收站删除）。
- AI CLI 集成：AI CLI 的屏幕检测与状态投影；Claude Code / Kimi /
  Codex / Pi / OpenCode 的 hook 安装；Claude Code 与 Codex 历史会话扫描；
  随包 runtime skill 自动投放。
- Runtime API：本机 loopback JSON Lines 控制面、版本化信封、`slterm ctl`
  与 `slterm env`、window/tab/pane/agent 资源动词、单实例移交。
- 文件与编辑：源码/文档/图片标签页、只读双栏 diff、git gutter、Markdown
  数学渲染、终端数学覆盖层、补全引擎。
- 系统集成：托盘、Windows toast 通知、任务栏进度、开机启动、窗口材质与
  壁纸、更新检查与后台下载。
- 安全：凭据存入 Windows 凭据管理器、设置写审计通道、口令保护的本地
  加密备份（严格白名单与敏感键双闸）。
- 打包：Inno Setup 安装器（用户级安装）、便携 zip、SHA256SUMS、体积预算
  钉测试与本地发布核验链。

### 修复 / Fixed

- Windows 10 握手污染：DA1 查询不再透传并污染输入行，修复新建终端蜂鸣、
  行首出现 `[?1;2c` 以及恢复会话命令被前缀破坏的问题。
- Windows 10 键事件输入：移除盲注 CPR，DSR 按传输层能力代答或剥离，修复
  敲键丢首字符的问题。
- Windows 10 全屏 CLI：捆绑新版 ConPTY 宿主，恢复全屏 TUI 的鼠标滚轮。
- git 并发：所有 git 子进程使用 `--no-optional-locks`，避免读操作触发
  index 锁或后台 gc。

### 改进 / Improved

- 单进程内直接驱动渲染与终端核心，去掉跨进程 IPC 往返；阻塞 I/O 与网络
  查询在后台执行，不占 UI 线程。
- 发布链以共享构建与包内清单为合同：安装器与便携包同源同 feature 图，
  新鲜度、版本回读与体积预算均在发布前机械校验。
