# 多 Dockview 实例（非单实例 + tab 切换）

Status: Accepted

每个操作页面拥有独立的 Dockview 组件实例。页面切换使用 CSS `display:none/block` 显隐，终端不销毁。

**根因**：xterm.js GitHub Issue #4978——`Terminal.open()` 不能在同实例上二次调用。如果用单 Dockview + tab 切换，切换页面时 Dockview 会卸载旧面板组件，再次切回时调用 `term.open()` 失败。这是 xterm.js 已知架构限制，不是 bug。

**为什么不是单页面应用？** 项目需要多操作页面并行工作（同时开多个终端会话用于不同项目目录）。

**备选方案**：维护全局 xterm.js 实例池，切换时 detach/attach DOM（Dockview 不支持外部的 DOM 生命周期管理，社区尝试均失败）。

**后果**：内存占用 = 页面数 × 每页面终端数。所有页面的终端进程并行运行。新页面首次切换有初始化开销（懒加载策略补偿）。
