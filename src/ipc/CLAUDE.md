# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 目录职责

`src/ipc/` 是前端唯一允许调用 Tauri `invoke` 的通信层。其他前端文件（组件、store、hook）**禁止**直接 `invoke` 或导入 `@tauri-apps/api/core`，必须通过本层封装函数访问后端。

## 模块映射

每个文件映射一个后端功能模块，命名一一对应：

| 文件 | 后端模块 | 封装的命令 |
|------|---------|-----------|
| `pty.ts` | `pty/` | `pty_spawn`, `pty_write`, `pty_resize`, `pty_kill`, `pty_reattach`, `get_windows_build_number` |
| `fs.ts` | `fs/` | `fs_read_file`, `fs_write_file`, `fs_read_dir`, `fs_create_dir`, `fs_delete`, `fs_rename` |
| `git.ts` | `git/` | `git_status`, `git_diff` |
| `settings.ts` | settings | `load_settings`, `save_settings` |
| `notify.ts` | `notify/` | `notify_watch`、`onFsEvent`（`listen("fs-event")` 封装） |
| `clipboard.ts` | Tauri plugin | 直接 re-export `@tauri-apps/plugin-clipboard-manager`。由 `keyboard.ts`（Ctrl+Shift+C/V）和 `useXterm.ts`（OSC 52 handler）消费 |
| `dialog.ts` | Tauri plugin | 直接 re-export `@tauri-apps/plugin-dialog` |
| `window.ts` | Tauri Window API | `registerCloseHandler` — 封装 `onCloseRequested` 关闭生命周期 |
| `shell.ts` | Tauri plugin | `@tauri-apps/plugin-opener` 的 `openUrl` re-export |
| `index.ts` | — | barrel export，统一对外暴露；含 `ping()` 健康检查命令 |

## 编码约定

- **invoke 单点**：`invoke` 调用只出现在本目录文件内（架构硬约束 #1）。
- **Channel 模式**：流式数据（如 PTY 输出）通过 `Channel<T>` 推送，调用方传入 `onOutput` 回调。
- **Event 模式**：`onFsEvent` 封装 Tauri `listen<FsEvent>("fs-event")`，返回 unsubscribe 函数。`registerCloseHandler` 封装 `getCurrentWindow().onCloseRequested` 生命周期。
- **类型对应**：封装函数的参数/返回值使用 `src/types/` 中的 DTO 类型，与 Rust 端 `snake_case` 字段对应。
- **thin wrapper**：clipboard、dialog 和 shell 是 Tauri 官方插件的直接 re-export，仅为了聚合到本层，不添加额外逻辑。新增 Tauri 插件导入遵循同一模式。
- **命名**：函数名 camelCase，对应的 Rust 命令为 snake_case（如 `pty_spawn` → `spawn()`）。
- **参数序列化**：`Uint8Array` 需转 `Array.from(data)` 再传给 `invoke`（pty.write）。

## 测试模式

测试文件：`src/__tests__/ipc-contract.test.ts`（47 用例）+ `ipc-ping.test.ts`（1 用例）。

### IPC 合约测试

核心思想：**用 `mockIPC` 拦截真实的 `invoke` 调用，验证每条封装函数的命令名、参数结构、返回类型和异常传播**。

### 四维验证

每个 IPC wrapper 测试覆盖四个维度：

| 维度 | 验证内容 | 示例 |
|------|---------|------|
| 命令名 | `invoke` 调用的 Tauri 命令名（snake_case） | `pty_spawn` 而非 `ptySpawn` |
| 参数结构 | 字段名、类型、值正确 | `args.request.panelId` 存在且为 `"p1"` |
| 正常返回 | mockIPC 返回模拟数据 → wrapper 正确透传 | `spawn()` 返回 `"session-01"` |
| 异常传播 | mockIPC throw → wrapper 不吞异常 | `expect(...).rejects.toThrow("conpty init failed")` |

### 关键模式

```typescript
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";

// 1. 注册命令处理器 + spy
const commandSpy = vi.fn();
mockIPC((cmd, args) => {
  commandSpy(cmd, args);
  if (cmd === "pty_spawn") return "mock-session-01";
  if (cmd === "fs_read_file") return "file content";
  throw new Error("unknown command");
});

// 2. 调用 IPC wrapper
const sessionId = await pty.spawn(request, onOutput);

// 3. 验证命令名 + 参数
expect(commandSpy).toHaveBeenCalledWith("pty_spawn", {
  request: { panelId: "p1", cwd: "C:\\test", cols: 120, rows: 40 },
  onOutput: expect.any(Channel),
});

// 4. 验证返回值
expect(sessionId).toBe("mock-session-01");
```

### Channel 绑定验证

PTY spawn 的 `onOutput` 回调必须绑定到 `Channel.onmessage`：

```typescript
const onOutput = vi.fn();
await pty.spawn(request, onOutput);
const channelArg = commandSpy.mock.calls[0][1].onOutput;
expect(channelArg.onmessage).toBe(onOutput);
```

### Uint8Array 序列化验证

`pty.write()` 必须将 `Uint8Array` 转为 `number[]`（Tauri IPC 不支持 TypedArray）：

```typescript
await pty.write("session-01", new Uint8Array([0x48, 0x69]));
expect(commandSpy).toHaveBeenCalledWith("pty_write", {
  sessionId: "session-01",
  data: [0x48, 0x69],  // number[]，非 Uint8Array
});
```

### notify mock 覆盖

`setup.ts` 全局 mock 了 `../ipc/notify`（防所有测试 import 时触发实际 listen）。ipc-contract 测试需要真实 `startWatch` → 在测试文件顶部用 `vi.mock` 覆盖全局 mock，`importOriginal` 获取原始实现。
