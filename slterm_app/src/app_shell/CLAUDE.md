# 应用壳组合层

> GPUI 壳的组合根：以 AppRoot 装配项目、运行面板、窗口与外观。

## 决策与红线

- **壳即前端**：壳内状态权威 = `AppRoot` 直持 `ProjectSnapshot` 与全部项目运行实体，无投影层。原因：单进程直持状态，避免跨层同步。边界：窗口装配只能选择一个运行根，不得保留旧 tabs/SplitTree 或第二套项目状态。
- **项目运行根边界**：`app_root` 以 `ProjectSnapshot` 为唯一项目、布局与逻辑终端事实源，强持全部项目运行实例。原因：项目切换必须保留隐藏项目终端，且不得形成新旧双树。边界：模块决策与验收见其目录文档；不得读取旧 session 或自建持久化通道。
- **诊断宏拦截**：`println!`/`eprintln!` 在本模块文本域内编译期报错；诊断统一走 `try_write_stderr`。原因：无效标准句柄上裸打印会 panic。
- **壳事件通道**：壳事件使用容量 64 的有界 `sync_channel`，生产发送统一为非阻塞 `try_send`，队列满只记录脱敏诊断。原因：托盘、通知、URL 与 writer 回调不得因事件洪峰无界增长或阻塞前台。边界：不在回调中阻塞等待队列；事件载荷不进入诊断文本。handover 经 application 单点 helper 投递，完成回执使用容量 1 的 `sync_channel`，投递与 AppRoot 回执均用 `try_send`；投递 Full/Disconnected 分别映射 QueueFull/Closing，回执接收端断开只放弃答复，不阻塞或 panic。
- **视觉语言**：仅暗色；Linear 为默认；暖黑明度阶梯、发丝线分隔、单强调色、等宽字体、无动效。原因：产品定位与「界面消失」的阅读优先。边界：色值以主题表为真值，不在壳内硬编码；状态用圆点而非 emoji。
- **布局单点**：运行与持久布局都以 `ProjectLayout` 为权威，只经其规范 DTO 转换。边界：禁止旧 session layout 或第二套序列化回流。
- **静默启动**：首窗 `show=false`/`focus=false` 创建；本层任何路径不得 show/activate/聚焦窗口。原因：启动不得抢焦点或闪现。
- **运行与退出事实**：`WorkspaceWriterRuntime` 在启动时登记并保留本次完整 `WorkspaceMetadata`（含 `run_id` 与 Unix epoch 毫秒创建时间），保留既有 `store_epoch` 并向启动恢复传递上一运行退出事实；`AppRootPersistence` 在既有启动恢复连接读取当前 `store_epoch` 并移交 AppRoot 供局部、项目与应用退出关闭，不另开读取通道。正常 ApplicationExit 完成局部段与最终 flush 后，由 GPUI 外 `GlobalCloseRuntime` 工作线程独占停止控制面、回收 writer 与失败转发器并提交 `clean_shutdown`，窗口收到 Completed 或 Abandoned 回执后才可销毁；控制面明确失败保留 writer 供显式重试，writer、转发器或回执结果未知时 fail-closed，任一失败均不写正常退出。原因：启动恢复、各作用域关闭、控制面与 Runtime 世代必须共享同一真实运行身份，最终状态不得先于其他持久提交或窗口销毁。边界：运行标记与最终冲刷共用同一工作区库，请求与每次回执均使用有界通道，GPUI 前台不等待控制面或 writer；最终提交失败由 GPUI 外工作线程以当前主窗 HWND 为 owner 弹出原生确认；只有用户选择取消才映射知情放弃，MessageBoxW 失败或其他返回值均上抛且不得产生 Abandoned 回执；放弃保持 `clean_shutdown=false` 并明确旧布局与残留快照可能恢复；异常窗口断连由工作线程兜底回收，仍先尝试 writer 最终冲刷与转发器 join，控制面失败不得伪造正常退出。
- **启动维护硬闸门**：主实例组合序固定为 OS 单实例锁 → 工作区库打开／`begin_run`／空注册表维护 → writer 与失败转发器 → 领域恢复 → `LocalControlService` → `GlobalCloseRuntime` → GPUI 窗口。维护未通过时不得创建 writer、转发线程与普通后台任务；维护或 writer 启动失败只记录脱敏诊断并进入独立诊断分支，已登记的本次运行保持 `clean_shutdown=false`，不得加载领域运行状态、启动控制面或创建正常工作区窗口；全局关闭所有者启动失败同样立即返回且不得创建窗口，已启动控制面、writer 与失败转发器必须显式 shutdown／join 后回收。原因：恢复事实未安全时继续启动会放大写入与运行副作用。边界：维护或 writer 失败只开放只读诊断与重试窗口，不得加载领域运行状态或启动控制面、PTY 与普通后台任务；诊断初始化只使用默认设置与暗色 token，禁止设置恢复写盘和壁纸加载；重试仅关闭诊断窗，外层在持续持有 OS 单实例锁的情况下从同一启动链重新执行，禁止以无 client 正常工作区替代。
- **控制面组合序**：仅 writer 可用时在 `AppRootPersistence` 恢复完成后启动 `LocalControlService`，复用 writer 本次元数据与同一壳事件发送端；启动失败记录脱敏错误并沿用降级 UI，不得伪造元数据或留下可用假端点。随后把服务与 writer 一并移交 `GlobalCloseRuntime`，正常路径在窗口销毁前显式停止服务并执行 writer 退出链，未启动服务视为清理成功；`application.run` 返回后只 join 已完成或断连兜底的工作线程，不再另开第二条关闭链。原因：发现记录只能指向完整真实运行，监听与发现撤销必须先于正常退出标记和窗口销毁。边界：OS 单实例锁仍由 `run()` 持有；本层不接第二实例 handover 入口。

## 验收与测试

- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 必测面：AppRoot 挂载与绘制、项目与布局单源、启动事件排水、关闭意图各提交阶段的真实子进程崩溃重开对账、事件通道容量与 AppRoot 销毁后接收端释放、激活与关闭生命周期、运行标记与最终冲刷成功/失败后的正常退出事实、全局控制面失败重试与窗口断连兜底回收、正常退出提交先于窗口销毁、静默启动不 show/activate、诊断宏拦截。
- 真实 GPU/桌面会话行为由构建产物人工验收，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增视图：先扩封闭 `ViewKey` 与单一注册点，再由 AppRoot 所有权链接线。
- 新增启动副作用：接入 `lib.rs::run()` 的同一启动链，不在子模块私开入口。
- 调整视觉语言：改主题表与语义 token，不在壳内写颜色字面量。
