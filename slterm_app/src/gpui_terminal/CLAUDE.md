# GPUI 终端视图

> 把核心终端 worker 接入 GPUI：固定代次命令绑定、不可变帧呈现与显式生命周期。

## 决策与红线

- **worker 单一所有权**：`AppRoot` 强持唯一 `TerminalRegistry`；Registry 独占逻辑终端到 `TerminalSession`、PTY 代次与进程身份的映射。原因：Panel、Runtime 与 View 分持会话会形成多个关闭源和 PID 复用错认。边界：View 只持固定 `TerminalTarget` 的弱 `TerminalBinding`，不得直持 `Term`、PTY 或子进程句柄；旧代次目标必须拒绝命令。
- **存活额度原子预检**：Registry 在调用启动或重连工厂前按 `process_live` 检查 64 个存活 PTY 上限；额度不足不得执行工厂、消耗新代次或改变既有目标，进程退出后同一记录可重试。原因：应用记录与真实进程必须共享同一容量事实，不能等创建成功后补救。边界：已退出和未启动记录不占额度；core 的全局创建锁仍作为最终并发闸门。
- **不可变帧呈现**：worker 在输出与显示状态边界发布带 revision 的 `TerminalFrame`；View 与绘制元素只读最新 `Arc` 帧，滚动、选区、resize 与配色经 worker 命令执行。原因：绘制阶段锁实时网格会把 PTY 解析与 GPU 呈现耦合，并让坐标映射读取过期状态。边界：paint 不锁 `Term`；显示命令只能在 worker 内修改网格。
- **退出不等于关闭**：进程退出只退休输入能力；显示 worker、事件订阅与最后画面继续服务滚动和选区，直到显式关闭。原因：用户退出 shell 后仍需阅读与复制输出。边界：事件泵不得因 `Exit` 自行终止；只有 View 销毁或 Registry 显式退休才结束对应生命周期。
- **关闭单点**：局部、批量、项目与应用关闭先按完整目标从 Registry 退休 worker，再在后台显式 `shutdown`／join；View Drop 只解除订阅，不杀 PTY。原因：隐藏项目与 View 析构不得绕过统一关闭协调。边界：GPUI 前台不得 join；重连换出的旧 worker 同样必须移交后台关闭。
- **有界事件传输**：每会话事件通道容量 256；帧与纯重绘可合并为最新帧，语义事件不得丢弃，满载由 worker 等待接收端释放容量。原因：输出洪流不能无界占用内存，也不能丢失命令边界或退出事实。边界：不得把语义事件降级为可覆盖状态。

## 验收与测试

- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support gpui_terminal:: -- --test-threads=1`。
- 必测面：启动成功才分配代次、重复启动不执行工厂、重连使旧目标失效、代次耗尽、64 个存活上限下启动／重连零副作用及退出后重试、进程身份包含 PID 与创建时间、退出后拒绝输入但继续交付显示帧、View Drop 不停止 worker、Registry 退休后显式 join、绘制快照不锁实时网格、事件合并不丢语义事件。
- 测试视图经 `restore_error`／`restore_exited` 构造后安装注册到同一 Registry 的内存 worker；除明确的真实 ConPTY 用例外，禁止拉取真实 PTY 或访问用户数据。

## 操作

- 新增终端显示能力：先扩 core 命令或不可变帧，再经 Binding 与事件泵接线，不在 View 直接操作实时网格。
- 新增 worker 生命周期入口：复用 Registry 与既有关闭协调器，明确后台 shutdown 所有权。
