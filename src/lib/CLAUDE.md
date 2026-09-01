# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

通用工具函数 + 共享 hooks + 错误边界 + E2E 门控 + 图标体系单点 + 统一浮层（确认弹窗/toast）+ 状态圆点。

## 关键约束与决策

### E2E 门控单一真值源

`src/lib/e2eEnabled.ts` 是 `__slterm_e2e_*` / `__e2e_*` helper 注入的唯一门控：

```ts
export const E2E_ENABLED =
  import.meta.env.DEV || import.meta.env.VITE_E2E === "1";
```

- 三态：dev serve → 开；`VITE_E2E=1`（`npm run build:e2e`）→ 开；生产发布构建 → `false`。
- `computeE2eEnabled(dev, viteE2e)` 是等价纯函数，**仅供单测**——`E2E_ENABLED` 必须内联 `import.meta.env` 字面量表达式，函数调用会阻碍 Rollup 跨模块 DCE，导致生产误带 helper。
- `main.tsx` 不引用常量（BOOT-01），其门控改为与本定义**逐字一致**的内联字面量，语义单点不变。

### 图标体系单点（IC-01/IC-06）

所有 lucide 图标经 `src/lib/icons.tsx` 引用，禁止其他文件直接 import `lucide-react`。CLI 品牌 logo 唯一映射在 `src/features/cliProfiles/`（`profile.iconSrc` + `public/cli-icons/<id>.png`）。

### 状态圆点单点（IC-03）

四态渲染一律经 `StatusDot` 组件（页签/导航树会话行/历史行），组件不得另画圆点或 emoji。`src/lib/agentStatus.ts` 仅存四态类型与 emoji 常量；事件→状态映射按 CLI profile 分发。

### 确认弹窗单点（OV-02）

确认语义一律经 `confirmDialog`（替换 Tauri `dialog.ask`）。`src/ipc/dialog` 已删 ask，只保留 open/save 原生文件对话框。

### panelId 生成/解析单点（B14）

`makeTerminalPanelId` 是终端 panelId 唯一生成入口，`parseTerminalPageId` 是唯一解析入口。格式协议仅定义于 `panelId.ts`。

旧恢复格式含 `Date.now` 数字段，语法切分无法判别；调用方应优先按已知 pageId 前缀匹配（TerminalPanel visible 判定与导航树定位），parse 仅兜底新格式。

### path.ts 规范

四个纯函数统一使用正斜杠 `/`，Windows 路径在比较前规范化：

- `normalizePath`：反斜杠→正斜杠。
- `basename`：提取文件名。
- `isChildOf`：规范化后前缀比较。
- `relativePath`：计算相对路径，不在子树返回 null。

所有函数不访问文件系统、不抛异常、空输入安全。

## 外部坑/红线

- **E2E_ENABLED 必须内联**：常量定义只能是 `import.meta.env` 字面量表达式，禁止调用 `computeE2eEnabled`。
- **路径函数纯性**：消费方依赖 `path.ts` 不抛异常、空输入安全的契约。
- **lucide 单点**：新增图标只追加到 `icons.tsx`。
- **状态圆点单点**：新增四态消费必须经 `StatusDot`。

## 测试模式

- **纯函数测试**：无 mock/jsdom/React 依赖。
- **E2E 门控**：断言真值表 + 常量与纯函数一致性；AST/正则断言 `E2E_ENABLED` 内联定义。
- **浮层单例**：ConfirmDialog/Toast 模块级单槽，测试经 `_reset` 隔离。

## 运行

```bash
npm test
```
