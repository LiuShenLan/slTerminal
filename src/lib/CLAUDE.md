# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

通用工具函数 + 共享 hooks + 错误边界 + E2E 门控开关

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | 公共 API 出口 |
| `path.ts` | 路径工具函数：`normalizePath` / `basename` / `isChildOf` / `relativePath` |
| `activePointer.ts` | `createActivePointer<T>()` 泛型工厂，供 terminal/editor 复用 |
| `useFontSizeWheel.ts` | Ctrl+Wheel 字体缩放共享 hook，供 useXterm/useCodeMirror 复用 |
| `ErrorBoundary.tsx` | React 错误边界组件 |
| `e2eEnabled.ts` | E2E helper 注入总开关：`E2E_ENABLED` 常量 + `computeE2eEnabled` 纯函数 |
| `injectScript.ts` | HTML 脚本注入纯函数：`injectScript(html, script, marker)` 向 HTML 字符串的 `</head>`/`<body` 前插入脚本，幂等（marker 检测），大小写不敏感，供 HtmlPanel 键盘转发 |

## e2eEnabled.ts — E2E 门控单一真值源

E2E 测试 helper（`__slterm_e2e_*` / `__e2e_*`）是否注入，由 `E2E_ENABLED` 单点门控，六个站点（`main.tsx`、`workspace/Workspace.tsx`、`panels/terminal/useTerminalInstance.ts`、`useXterm.ts` ×3）统一引用。

```ts
export const E2E_ENABLED =
  import.meta.env.DEV || import.meta.env.VITE_E2E === "1";
```

- **三态**：dev serve `DEV=true` → 开；E2E 构建 `VITE_E2E=1`（`npm run build:e2e`）→ 开；生产发布构建两者皆 false → 编译期折叠为 `false`，helper 整块 tree-shake（二进制无测试后门）。
- **为何需要 VITE_E2E**：`tauri build` 前端恒走 `vite build`（production，`DEV=false`，与 `--debug`/`--mode` 无关），若只靠 DEV，E2E 二进制会丢 helper → wdio 卡"Workspace 未就绪"。详见 `@../../e2e-tests/CLAUDE.md`。
- **DCE 关键**：`E2E_ENABLED` 必须**内联** `import.meta.env` 表达式（编译期字面量 → Rollup 常量折叠）。`computeE2eEnabled(dev, viteE2e)` 是等价纯函数，仅供单测全表覆盖——**勿**让常量调用它（函数调用阻碍跨模块 DCE，会使生产误带 helper）。
- **守卫**：`e2e-enabled.test.ts` 断言真值表 + 常量与纯函数一致性；CI 有生产 dist grep step 强制"生产不含 helper"。

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
