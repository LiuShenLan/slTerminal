# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

通用工具函数 + 共享 hooks + 错误边界

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | 公共 API 出口 |
| `path.ts` | 路径工具函数：`normalizePath` / `basename` / `isChildOf` / `relativePath` |
| `activePointer.ts` | `createActivePointer<T>()` 泛型工厂，供 terminal/editor 复用 |
| `useFontSizeWheel.ts` | Ctrl+Wheel 字体缩放共享 hook，供 useXterm/useCodeMirror 复用 |
| `ErrorBoundary.tsx` | React 错误边界组件 |

## path.ts — 路径规范

四个纯函数，统一使用正斜杠 `"/"` 作为分隔符，Windows 路径在比较前规范化：

| 函数 | 行为 | 示例 |
|------|------|------|
| `normalizePath(p)` | 反斜杠→正斜杠 | `"D:\\a\\b"` → `"D:/a/b"` |
| `basename(p)` | 提取文件名 | `"D:/a/b/index.ts"` → `"index.ts"` |
| `isChildOf(file, root)` | file 是否在 root 子树中 | 规范化后前缀比较 |
| `relativePath(file, root)` | 计算相对路径，不在子树中返回 null | `"D:/a/b/c.ts"` 相对 `"D:/a"` → `"b/c.ts"` |

所有函数不访问文件系统，不抛异常，空输入安全。

## 测试模式

测试文件：`src/__tests__/path.test.ts`（27 用例）。

### 纯函数测试

- **无 mock、无 jsdom、无 React**：纯数据转换，直接调用断言
- **每函数独立 describe**：`normalizePath`/`basename`/`isChildOf`/`relativePath` 各一个 `describe` 块
- **边界覆盖**：空字符串、`null`/`undefined`、正斜杠 vs 反斜杠、不同盘符、同名前缀、结尾斜杠
- **对应关系**：测试用例参照 `path.ts` 路径规范表格中的示例编写

### 通用模式

纯工具函数（如 `path.ts`、`error-boundary.test.tsx`）优先于 UI 集成测试。新增纯函数时在 `src/__tests__/` 下创建对应测试文件。
