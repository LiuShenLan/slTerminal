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
| `notify.ts` | `notify/` | `fs_watch` |
| `clipboard.ts` | Tauri plugin | 直接 re-export `@tauri-apps/plugin-clipboard-manager`。由 `keyboard.ts`（Ctrl+Shift+C/V）和 `useXterm.ts`（OSC 52 handler）消费 |
| `dialog.ts` | Tauri plugin | 直接 re-export `@tauri-apps/plugin-dialog` |
| `index.ts` | — | barrel export，统一对外暴露 |

## 编码约定

- **invoke 单点**：`invoke` 调用只出现在本目录文件内（架构硬约束 #1）。
- **Channel 模式**：流式数据（如 PTY 输出）通过 `Channel<T>` 推送，调用方传入 `onOutput` 回调。
- **类型对应**：封装函数的参数/返回值使用 `src/types/` 中的 DTO 类型，与 Rust 端 `snake_case` 字段对应。
- **thin wrapper**：clipboard 和 dialog 是 Tauri 官方插件的直接 re-export，仅为了聚合到本层，不添加额外逻辑。新增 Tauri 插件导入遵循同一模式。
- **命名**：函数名 camelCase，对应的 Rust 命令为 snake_case（如 `pty_spawn` → `spawn()`）。
- **参数序列化**：`Uint8Array` 需转 `Array.from(data)` 再传给 `invoke`（pty.write）。
