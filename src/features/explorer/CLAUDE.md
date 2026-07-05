# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件浏览器（ExplorerPanel）——左侧面板，展示当前活跃项目的文件树，支持 CRUD 操作、Git 状态着色和文件系统事件增量刷新。

## 架构决策

**Generation 异步取消**：切换项目页面时 `rootPath` 变化，`useFileTree` 的 effect 发起新 `loadRoot()` + `gitStatus()` 异步请求。旧 `rootPath` 的请求可能在新请求之后才返回，导致 `setRootNodes` 写入过期数据。`genRef` 计数器每次 `rootPath` 变化递增，旧请求的回调检查 `gen !== genRef.current` 后丢弃结果。

- 仅 `rootPath` effect 中的调用使用 generation
- `refreshExpanded`（CRUD 操作 / fs-event / file-saved 回调）不传 gen——它们操作的是当前页数据
- `loadRoot(gen?)` 的 `gen` 参数是可选的后向兼容接口

**`useFileTree` 自包含加载**：`rootPath` 变化时 `useFileTree` 内部 effect 自动调用 `loadRoot()` + `gitStatus()`。ExplorerPanel 只负责调用 CRUD 操作后的 `refresh()`，**不在 `rootPath` 变化时重复刷新**（历史重复 effect 已删除）。

## 文件

| 文件 | 职责 |
|------|------|
| `ExplorerPanel.tsx` | React 容器组件：活跃项目推导、文件树渲染、CRUD 事件处理、`fs_watch` 启动 |
| `useFileTree.ts` | 文件树数据 hook：`loadRoot` / `loadDirectory` / `toggleExpand` / `refreshExpanded` / generation 取消 |
| `FileTree.tsx` | 递归树组件：节点渲染、git 状态着色、右键菜单 |
| `FileIcon.tsx` | 文件图标映射（扩展名→emoji） |

## 关键集成点

- **`src/ipc/fs.ts`** — `readDir` / `writeFile` / `createDir` / `deleteEntry` / `rename`
- **`src/ipc/git.ts`** — `gitStatus` 着色
- **`src/ipc/notify.ts`** — `startWatch` 启动后端监听 + `onFsEvent` 增量刷新
- **`src/stores/projects.ts` + `src/stores/layout.ts`** — 活跃项目 `rootPath` 推导
- **`src/workspace/titleManager.ts`** — 文件打开时计算编辑器页签标题 + 去重

## IPC 约束

- 所有文件操作经 `src/ipc/` 层调用，禁止组件内直接 `invoke`
- `onFsEvent` 通过 Tauri `listen()` 全局订阅，`useFileTree` 内 200ms debounce
