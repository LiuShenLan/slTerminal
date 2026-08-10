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
| `agentStatus.ts` | 四态类型与 emoji 常量（MC-401 迁移）：`AgentStatus` 类型（`working`/`attention`/`done`/`error`）+ `STATUS_EMOJI` 常量（⚡🟡✅❌）+ `getStatusIcon(status)` 纯函数。**事件→状态映射已随 MC-401 迁出**至 CLI profile hooks 能力（`profiles/claude/strategies.ts` 的 `eventToStatus`，经 `profile.hooks` 委托分发），lib 层不再含 claude 事件名字面量 |
| `cliIcons.ts` | CLI 品牌 logo 注册表单例（F9）：`CliIconRegistry`（`register`/`match` 首 token/`getSrc`/`_reset`）+ 内嵌注册 claude → `/cli-icons/claude.png`。三处 emoji 状态指示（页签/活跃/历史）消费。**新增编码 CLI 两步**：`public/cli-icons/<命令>.png` 放图（32×32 透明底，渲染 16×16，随 frontendDist 内嵌 exe，同源 `'self'` 加载）+ 本文件末尾追加一行 `register({ command, src })` |
| `panelId.ts` | 终端 panelId 解析单点：`parseTerminalPageId(panelId)` → pageId \| null（≥3 段 + 首段 `terminal` + 末段全数字） |

## e2eEnabled.ts — E2E 门控单一真值源

E2E 测试 helper（`__slterm_e2e_*` / `__e2e_*`）是否注入，由 `E2E_ENABLED` 单点门控，五个站点（`workspace/Workspace.tsx`、`panels/terminal/useTerminalInstance.ts`、`useXterm.ts` ×3）统一引用。`main.tsx` 不引用常量（BOOT-01 执行期修正，修正记录 4——rolldown 不折叠跨模块常量，引用 `E2E_ENABLED` 会使 helpers chunk 残留生产 dist、CI 生产剥离守卫必 fail），其门控改为与 `e2eEnabled.ts` 定义**逐字一致**的内联字面量 `import.meta.env.DEV || import.meta.env.VITE_E2E === "1"`，门控语义单点不变。

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

## 关键约束

- **E2E_ENABLED 必须内联**：`import.meta.env` 字面量表达式（编译期折叠），禁止常量调用 `computeE2eEnabled`——函数调用阻碍 Rollup DCE，生产会误带 helper（守卫：`e2e-build-config.test.ts`）
- **四态映射单点**：F3 事件→状态唯一映射按 CLI profile 分发（`profile.hooks.eventToStatus`，claude 实现见 `profiles/claude/strategies.ts`），组件不得另建映射；`agentStatus.ts` 仅保留四态类型与 emoji 常量
- **CLI 图标映射单点**：`cliIcons.ts` 是 CLI → 品牌 logo 唯一映射（F9），新增 CLI 在此注册（每 CLI 一行 register + public/cli-icons 放图），组件不得另建映射
- **panelId 解析单点**：`parseTerminalPageId` 是终端 panelId 唯一解析入口
- **路径函数纯性**：`path.ts` 四函数不访问文件系统、不抛异常、空输入安全（消费方依赖此契约）

## 测试模式

测试文件：`src/__tests__/path.test.ts`（27 用例）、`panelId.test.ts`（5 用例）、`cli-icons.test.ts`（12 用例，F9）、`inject-script.test.ts`（21 用例）、`agent-status-lib.test.ts`（6 用例，MC-401 迁移——事件映射用例随实现迁入 `cli-profile-claude.test.ts`）、`e2e-enabled.test.ts`（9 用例，含 it.each 展开口径，见 `.claude/test-inventory.md`）、`e2e-build-config.test.ts`（8 用例，IHE-04）、`error-boundary.test.tsx`（5 用例，含 IHE-05 `variant="inline"` 分支）。

### path.test.ts

- **无 mock、无 jsdom、无 React**：纯数据转换，直接调用断言
- **每函数独立 describe**：`normalizePath`/`basename`/`isChildOf`/`relativePath` 各一个 `describe` 块
- **边界覆盖**：空字符串、`null`/`undefined`、正斜杠 vs 反斜杠、不同盘符、同名前缀、结尾斜杠
- **对应关系**：测试用例参照 `path.ts` 路径规范表格中的示例编写

### panelId.test.ts

纯函数测试：`parseTerminalPageId` 全分支——正常解析（`terminal-page1-0`→`"page1"`）、含连字符 pageId（`terminal-my-page-2`→`"my-page"`）、尾段非数字→null、非 terminal 前缀→null、两段→null。

### E2E 门控测试（e2e-enabled.test.ts + e2e-build-config.test.ts）

- `e2e-enabled.test.ts`：`computeE2eEnabled` 真值表 + 常量与纯函数一致性
- `e2e-build-config.test.ts`（IHE-04）：AST/正则断言 `E2E_ENABLED` 定义为内联 `import.meta.env` 字面量表达式（不得调用 `computeE2eEnabled`）——若被包成函数调用，Rollup 无法 DCE，生产误带测试后门且 L2 不报警

### 通用模式

纯工具函数（如 `path.ts`、`error-boundary.test.tsx`）优先于 UI 集成测试。新增纯函数时在 `src/__tests__/` 下创建对应测试文件。
