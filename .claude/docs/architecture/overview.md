# 架构总览与依赖边界

> 单进程 GPUI 模块化单体：应用壳拥有全部 UI 状态与 OS 访问；core 提供无 UI 依赖的领域能力；hook 是独立小进程桥。

## 决策与红线

- **单进程单窗口**：一个进程、一个窗口，无 WebView、无 IPC、无 Node 运行时。原因：GPU 加速、状态单源、消除跨进程跟随延迟与投影层。边界：不得引入第二个 UI 进程或把业务逻辑放进渲染层。
- **workspace 成员与 ownership**：

  | crate | owns | must not become |
  |---|---|---|
  | `slterm_app`（lib+bin `slterm`） | GPUI 壳、workspace/tab/pane 生命周期、设置 UI 与写通道、平台适配、Runtime API、AI 集成、文件/编辑器、i18n、打包入口 | 不承载 VT/PTY 解析；不把业务逻辑塞进渲染层；不绕过领域通道直写磁盘/凭据 |
  | `slterm_terminal` | 网格、VT 解析、PTY 行为、输入编码、选择与回放、渲染快照 | 不依赖产品视图、窗口状态或 GPUI 类型 |
  | `slterm_settings` | 设置值模型、JSON 值与键注册表、路径权威、主题色表与 WCAG 校验 | 零生产依赖；不持写通道硬机制 |
  | `slterm_split` | 分屏树、几何切割、拖拽比例与导航纯函数 | 零生产依赖；不出现 UI 框架类型 |
  | `slterm_completions` | 补全匹配引擎与来源合同 | core 层；不依赖 app 或渲染包 |
  | `slterm_hook`（bin `slterm-hook`） | AI CLI hook 小进程桥：有界转发后退出 | 零生产依赖；必须 exit 0、快速、隐形，禁阻塞调用方 |

- **依赖方向**：application → core 单向；`slterm_hook` 不被 app 依赖；`renderer_packages`（gpui/gpui_platform/gpui-component/winit 等）禁入 core/hook 生产依赖。原因：核心层可独立测试与复用，渲染依赖误入即失去边界。边界：由 `architecture/dependencies.toml` 与 `scripts/check_architecture.py` 机械强制；根 workspace 成员与策略键集全等。
- **跨领域类型锚点**：pane/tab 标识与分屏树归 `slterm_split`；设置/主题类型归 `slterm_settings`；终端网格/OSC/快照类型归 `slterm_terminal`；补全值族归 `slterm_completions`；通用错误与品牌常量归 `slterm_app::error`/`slterm_app::brand`。边界：他处只许 `use` 或经 facade 传参，禁重复定义、别名漂移与字段语义改写；新增跨领域类型先登记归属。
- **错误信封**：统一使用 `slterm_app::error::AppError`；变体与 `From` 转换以类型定义为准。原因：调用点语义统一，领域不得另立 envelope。边界：错误消息是用户可见语义，禁止携带凭据值；领域失败经统一通用出口，不新增平行错误类型。
- **平台分支收敛**：业务 `#[cfg(windows)]` 仅允许出现在 PTY/平台封装模块；终端 core 的 Windows 适配收在 `tty/windows`。原因：平台差异单点可达，便于替换与测试。边界：业务模块不得散落平台条件编译。
- **模块不穿透**：crate 内模块共享只经公开小接口/类型锚点；跨 crate 共享只经 core crate 公开 API。原因：单一执行体与单一读取通道，防止状态双源。边界：禁领域自建第二套读取/执行/写盘通道。
- **数据目录与命名**：默认 `%APPDATA%\slterm`；`SLTERM_CONFIG_DIR` 优先，空串视为未设置；回退链 APPDATA → USERPROFILE → temp。路径推导唯一权威在 `slterm_settings::paths`。文件名去品牌：`settings.json`、`session.json`、`providers.json`、`history.jsonl`、`directory_history.json`、`runtime.port`、`themes/*.slterm-theme.json`。原因：目录名已带品牌，文件名不重复品牌，避免下次改名产生迁移链。边界：新版只读写新目录，旧版 exe 同级数据不看不迁；任何新持久化文件沿用去品牌命名与原子写纪律。
- **架构门禁**：`architecture/file-budgets.txt` 行数棘轮、`architecture/dependencies.toml` 依赖方向、`scripts/check_architecture.py` 机械校验，git hooks 常挂。原因：边界只有机械化才可持续。边界：新增 crate/依赖必须先过策略与预算；renderer 依赖误入 core 即红。

## 验收与测试

- `python scripts/check_architecture.py` 零 ERROR。
- `scripts/tests/test_architecture_governance.py` 校验工具链钉版与许可证合规。
- 依赖方向、零生产依赖、workspace 成员键集由架构门禁覆盖；类型锚点与错误信封由各 crate 的编译边界约束。

## 操作

- 完整命令矩阵见 `.claude/docs/operations/commands.md`。
- 上游基线、fork 与旧栈删除沿革见 `.claude/docs/architecture/provenance.md`。
