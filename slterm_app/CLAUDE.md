# slterm_app（application 层）

> GPUI 壳与一切应用编排：窗口/工作区、设置 UI 与写通道、AI CLI 集成、文件与编辑器、平台适配、i18n 与打包入口。

## 决策与红线

- **不做第二执行体**：git、文件树、补全、AI 会话等每个领域只有一个执行/解析入口；面板与后台任务都消费同一入口。原因：双执行体会产生状态双源与语义漂移。边界：不得在面板侧 spawn 子进程或复刻解析逻辑。
- **壳即前端**：GPUI 前台直接持有实体与展示态，无跨进程投影层；状态变更后显式通知/重绘。
- **前台纪律**：阻塞 I/O、网络、子进程等待不得在 GPUI 前台线程执行；走 background executor 或专用线程，结果回传前台。原因：前台阻塞会冻结整个窗口。边界：任何 `await` 子进程/网络的代码都必须在后台执行体。
- **启动链单点**：启动链单点在 `lib.rs` 的 `run()`；`main.rs` 只承载 Windows 子系统选择与委托，保持薄壳。原因：入口分叉会产生两套初始化顺序。边界：新增启动副作用必须接入同一链。
- **退出链单点**：关窗/托盘退出/系统关机都走同一「终局快照 + 退出」链；脏文件确认不得被旁路。原因：任一退出路径绕过确认都会丢用户数据。
- **后台任务元数据单点**：`background_tasks` 注册表持任务 id、标题、边界、默认值、`schedule` 与执行体；设置页与调度器只读该表。`TaskSchedule` 表达 `Interval`/`Once`/`OnDemand` 三种驱动方式，共用订阅生命周期、重入闸门与失败策略。原因：任务元数据双源必然漂移。边界：新增任务 = 注册表一条元数据 + 一端执行体，框架零改动。
- **单实例启动**：GUI 启动须先持有 `platform::instance` 的 OS 独占锁；无移交服务时第二实例必须退出，不得降级为第二窗口。原因：单窗口产品约束不能依赖已删除的控制面。边界：本地维护子命令在锁前分流。
- **设置写盘单点**：设置写盘单点 = `settings_store`（白名单、浅合并、原子写、`.bak`、损坏处理）；所有持久化经 `atomic_file`，不裸写。原因：并发写与损坏处理必须集中。边界：前端消费型配置走通用段写；后端消费型配置走域专用命令；禁止自建第二写通道。
- **审计通道**：安全写事件只经 `audit` facade（tracing target `audit`）；事件字段只含 `operation/path/source/result` 与 `expected/actual` 指纹；不记文件内容或凭据值。
- **凭据红线**：凭据只经 `secrets`/Windows 凭据管理器门面；供应商不得从 CLI 配置或环境变量读取认证材料。专用 token 仅在凭据域与请求组装点间经 `SecretBytes` 传递，不进入 DTO/快照/日志，错误消息禁插值。测试夹具一律假值。
- **加密备份边界**：备份白名单即安全边界；只读写登记的类别与主题目录。凭据值、AI 会话库、任意 home 路径永不进出包。恢复序固定为「完整鉴权 → 全量校验 → 落盘」，任一条不过整单拒写。敏感键导出剥除 + 导入复验双闸；备份魔数同时作加密 AAD。
- **agent 目录沙箱**：agent 全局目录由后端静态表定义，前端不能注入任意路径；沙箱放行域 = project_root ∪ agent 目录集，canonicalize 双向比较；pinned watcher 在项目切换时不被暂停。原因：agent 配置文件在项目根外，但读取必须仍走同一沙箱命令族。边界：不得为 agent 文件建第二套读取通道。
- **主题与外观**：内置主题为暗色目录，Linear 为默认，未知/退休 id 回退 Linear；主题校验在 `slterm_settings`。`BlurMode` 与透明度是外观设置，平台调用只在平台封装模块，静默启动不得闪现/抢焦点。
- **i18n 合同**：`en-US`/`zh-CN` 目录 JSON 构建期展平为 typed 键；翻译键集是硬合同（缺键/多余键构建期红）。运行时只经 `Message` 查询，禁止字符串键、英文文案反查或双语即时分支等第二翻译通道。`LanguagePref` 支持系统/英文/中文，系统按 locale 协商，未知 locale 回退英文。
- **双数据根**：启动入口只读取一次 `SLTERM_CONFIG_DIR`、`SLTERM_LOCAL_DIR`，构造不可变 `AppPaths`；覆盖值仅接受绝对路径，空串与相对路径明确失败。默认经 Known Folder API 得 `%APPDATA%\slterm` 与 `%LOCALAPPDATA%\slterm`。设置、设置备份与主题归配置根；数据库、会话、历史、日志、实例锁、恢复材料与更新材料归本地根。生产调用点只读启动缓存，core 不读数据根环境。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。UI 用例 `#[cfg(all(test, feature = "gpui-test-support"))]` 双门控；feature 未开时整体不编译，生产构建不带测试夹具。
- 启动链：`cargo test -p slterm --test windows_console_startup -- --test-threads=1`。
- 平台/网络/桌面会话依赖项登记 `.claude/docs/testing/exemptions.md`；不登记即违规。

## 操作

- 模块细节导航：设置页、工作区、文件编辑器、终端视图、数学渲染、平台集成、更新链等各自目录下的 `CLAUDE.md` 是该域的真值源；本文件只保留跨模块纪律。新增模块目录时同步建立其 `CLAUDE.md`。
- 新增后台任务：在注册表登记元数据与执行体，配置经注册表命令写入。
- 新增持久化：经 `atomic_file` 与 `settings_store`/`audit` 包装；文件名去品牌。
