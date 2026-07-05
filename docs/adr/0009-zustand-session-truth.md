# Zustand 双消费模式 + 会话单点

Status: Accepted

所有全局状态使用 Zustand 管理。React 组件通过 selector 订阅，非 React 代码（事件处理、清理钩子）通过 `.getState()` 快照读取。会话元数据只存在 `sessions` store 中。

**为什么 PTY 进程映射不走 Zustand？** PTY 输出每秒数百次 `PtyEvent` 推送——如果把进程引用放 Zustand，每一次推送都可能触发 React 重渲染（即使 selector 不选它，Zustand 的引用比较也会因对象变更而失效）。终端面板内部用模块级 `Map` 管理 PTY 进程引用，完全绕开 React 渲染循环。

**为什么是双消费模式？** 事件处理器（如 2s debounce 保存）需要读当前状态但不触发渲染——Zustand 的 `.getState()` 在没有 selector 开销的情况下返回即时快照。React hooks + selector 则提供渲染绑定和精确的重渲染控制。

**备选方案**：全部放 React Context（深度嵌套面板下的重渲染风暴）；Redux Toolkit（过度工程化，项目中只有 3 个 store）。

**后果**：store 不允许互相导入（防止隐式依赖）。持久化委托给 IPC 层，不在 store 内直接调用 Tauri invoke。
