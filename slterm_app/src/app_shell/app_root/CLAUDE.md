# AppRoot 运行根

> 以唯一项目快照协调全部项目的运行面板、瞬时焦点与终端生命周期。

## 决策与红线

- **项目与布局单源**：`AppRoot` 直持 `ProjectSnapshot`；`projects` 只是全部 `ProjectView` 的强实体引用表，不复制项目注册、布局或逻辑终端记录。原因：运行实例不得成为第二项目或布局事实源。边界：`ProjectView` 只持 `PanelHost` 与项目内瞬时焦点，`PanelHost` 只持 `PanelId`、`ViewKey` 与 `TerminalView`。
- **候选原子提交**：新增与拆分先由快照产生候选，再执行一次可失败启动并登记实例，最后提交候选；关闭先产生候选，提交后移除订阅与实例并显式 shutdown。原因：布局、逻辑终端记录与 PTY 不得出现半提交。边界：启动失败不保留 `session=None` 的占位面板，正式快照与实例表不变；PanelId 失败消耗后也不复用。
- **可失败启动接缝**：终端先经 `TerminalView::launch` 冻结配置并启动 PTY，成功后才经 `from_launched` 构造实体。原因：GPUI 实体构造不能直接返回启动 `Result`，失败不得先挂实体再补救。边界：成功启动只执行一次；非 Unicode Shell 路径在参数组装前拒绝，不做有损转换。
- **项目切换保活**：所有项目的 TerminalView 与 PTY 强持于各自 ProjectView；切换只调整 `output_visible` 与焦点，隐藏项目的会话泵、输出解析和回滚继续运行，重新可见时主动补帧。原因：项目导航不得重建终端或丢失后台输出。边界：显式新建只面向活跃项目；隐藏项目 Launch 的懒启动由恢复协调负责，不在显式命令中暗中 spawn。
- **退出与关闭分离**：TerminalView 的 Exited 只把逻辑记录改为 `RunIntent::Exited`，保留面板、实体与最后画面；显式 close 才释放实例。原因：进程退出不是用户关闭面板。边界：重复或迟到的退出事件不得关闭其他面板；未知运行身份不猜快照目标。
- **无旧恢复旁路**：本模块不读取、迁移或清理旧 session 与磁盘历史材料，不与旧 tabs/SplitTree 同步双写。原因：新根必须只有一条布局与生命周期通道。边界：生产恢复只能由启动协调显式注入一致快照，实体层不得自行读盘。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests --features gpui-test-support app_shell::app_root::tests:: -- --test-threads=1`。
- 必测面：添加项目不自动建终端、启动失败原子性、新建/拆分/激活复用实体、关闭释放与空项目保留、项目切换保活、隐藏事件继续处理、Exited 留存、重复根激活、非 Unicode Shell 拒绝。
- 测试启动器使用内存 `test_session`，禁止拉取真实 PTY 或访问用户数据。
