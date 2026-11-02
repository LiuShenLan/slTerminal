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
- **运行与退出事实**：`WorkspaceWriterRuntime` 在启动时登记并保留本次完整 `WorkspaceMetadata`（含 `run_id` 与 Unix epoch 毫秒创建时间），保留既有 `store_epoch` 并向启动恢复传递上一运行退出事实；应用退出只在控制面清理成功、最终冲刷与失败转发回收均成功且零失败后提交 `clean_shutdown`，任一失败保持未正常退出。原因：启动恢复、控制面与 Runtime 世代必须共享同一真实运行身份，最终状态不得先于其他持久提交。边界：运行标记与最终冲刷共用同一工作区库，不另开退出记录通道；控制面清理失败仍必须完成 writer 最终冲刷与转发器 join，writer、转发器或最终提交失败不得伪造正常退出。
- **启动维护硬闸门**：主实例组合序固定为 OS 单实例锁 → 工作区库打开／`begin_run`／空注册表维护 → writer 与失败转发器 → 领域恢复 → `LocalControlService` → GPUI 窗口。维护未通过时不得创建 writer、转发线程与普通后台任务；维护或 writer 启动失败均只记录脱敏诊断并立即返回，已登记的本次运行保持 `clean_shutdown=false`，不得加载领域运行状态、启动控制面或创建窗口。原因：恢复事实未安全时继续启动会放大写入与运行副作用。边界：本切片失败即无窗口；只读诊断 UI 由后续切片实现，不得以旧无 client 降级路径替代。
- **控制面组合序**：仅 writer 可用时在 `AppRootPersistence` 恢复完成后、`application.run` 前启动 `LocalControlService`，复用 writer 本次元数据与同一壳事件发送端；启动失败记录脱敏错误并沿用降级 UI，不得伪造元数据或留下可用假端点。`application.run` 返回后先在 GPUI 外显式关闭服务，再执行 writer 退出链；未启动服务视为清理成功。原因：发现记录只能指向完整真实运行，监听与发现撤销必须先于正常退出标记。边界：OS 单实例锁仍由 `run()` 持有；本层不接第二实例 handover 入口。

## 验收与测试

- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 必测面：AppRoot 挂载与绘制、项目与布局单源、启动事件排水、事件通道容量与 AppRoot 销毁后接收端释放、激活与关闭生命周期、运行标记与最终冲刷成功/失败后的正常退出事实、静默启动不 show/activate、诊断宏拦截。
- 真实 GPU/桌面会话行为由构建产物人工验收，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增视图：先扩封闭 `ViewKey` 与单一注册点，再由 AppRoot 所有权链接线。
- 新增启动副作用：接入 `lib.rs::run()` 的同一启动链，不在子模块私开入口。
- 调整视觉语言：改主题表与语义 token，不在壳内写颜色字面量。
