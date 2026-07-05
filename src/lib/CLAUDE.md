# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

通用工具函数——纯函数，无副作用，不依赖 React 或任何外部状态。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | 公共 API 出口 |
| `path.ts` | 路径工具函数：`normalizePath` / `basename` / `isChildOf` / `relativePath` |

## path.ts — 路径规范

四个纯函数，统一使用正斜杠 `"/"` 作为分隔符，Windows 路径在比较前规范化：

| 函数 | 行为 | 示例 |
|------|------|------|
| `normalizePath(p)` | 反斜杠→正斜杠 | `"D:\\a\\b"` → `"D:/a/b"` |
| `basename(p)` | 提取文件名 | `"D:/a/b/index.ts"` → `"index.ts"` |
| `isChildOf(file, root)` | file 是否在 root 子树中 | 规范化后前缀比较 |
| `relativePath(file, root)` | 计算相对路径，不在子树中返回 null | `"D:/a/b/c.ts"` 相对 `"D:/a"` → `"b/c.ts"` |

所有函数不访问文件系统，不抛异常，空输入安全。
