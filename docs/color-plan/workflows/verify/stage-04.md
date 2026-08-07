# Stage 04 逐项验证断言（唯一真值源）

> 中间态口径：本 Stage 后启动链为目标态；全部代码 Stage 完成。断言按目标态写（checklist 修正记录 2）。
> 修正记录（执行期，Stage 04 首验）：断言 1 目标态由「静态 import 恰 3 个（含 `./lib/e2eEnabled` 深导入）」修正为「静态 import 恰 2 个 + E2E 门控内联字面量」——实现实证：rolldown 不做跨模块常量折叠，main.tsx 引用 `E2E_ENABLED` 常量会使 helpers chunk 残留生产 dist（CI 生产剥离守卫必 fail）；门控改为与 e2eEnabled.ts 定义逐字一致的内联 `import.meta.env.DEV || import.meta.env.VITE_E2E === "1"` 后 DCE 实证零泄漏（A/B 对比）。意图（静态面最小 + setActive 前不触碰 facade + DCE）达成且静态面更小（2<3）。详见 checklist 修正记录 4。断言 8 允许清单扩展：+ `docs/color-plan/execution-plan.md`（进度簿记，同 Stage 02 先例）。

## 断言清单

| # | 对应项 | 断言 | 检验方法 |
|---|--------|------|---------|
| 1 | BOOT-01 | `src/main.tsx` 静态 import 恰 2 个（`react`、`react-dom/client`）；E2E 门控为内联字面量 `import.meta.env.DEV || import.meta.env.VITE_E2E === "1"`（与 e2eEnabled.ts 定义逐字一致，含偏离原因中文注释）；其余 import 均为动态 `import(...)` | Grep `^import ` 计数 + Read |
| 2 | BOOT-01 | main.tsx 无 `from "./lib"`（barrel）与 `from "./theme"` 静态 import；无 `import "./App.css"` | Grep |
| 3 | BOOT-01 | 启动序列顺序（Read 全文确认调用先后）：① IPC wait + fail-safe → ② loadSettings + setActive → ③ ROOT_CSS_VARS 注入 → ④ E2E helpers 注入 → ⑤ await import("./App") + render；fail-safe 的 `#1e1e1e`/`#f44747` 字面量保留不变 | Read 顺序断言 |
| 4 | BOOT-01 | setActive 实参来自 loadSettings 结果的 colorScheme 字段；loadSettings 失败有 `.catch` 降级（不阻断启动）——语义式：Read 确认存在 catch 分支，非裸 await | Read |
| 5 | BOOT-01 | `src/__tests__/bootstrap.test.ts` 含 `../ipc/settings` mock（loadSettings resolve null）；原有 `../App`、`../App.css` mock 保留 | Grep + Read |
| 6 | BOOT-02 | `src/App.tsx` 含 `import "./App.css"`，且在 `dockview-react/dist/styles/dockview.css` import 之后 | Read 顺序 |
| 7 | BOOT-03 | `src/App.css` 无 `--sl-fg-primary:` 与 `--sl-fg-secondary:` 的 hex 定义行；`var(--sl-fg-primary)` 与 `var(--sl-bg-primary)` 引用保留 | Grep |
| 8 | 中间态 | 本 Stage diff 仅含：`src/main.tsx`、`src/App.tsx`、`src/App.css`、`src/__tests__/bootstrap.test.ts` + `docs/color-plan/execution-plan.md`（进度簿记） | git diff --name-only HEAD |
| 9 | 构建 | `npx vite build` 成功（动态 import 图构建级验证） | 门禁产出 |

## 全量测试（全部通过为门禁）

- `npx tsc --noEmit` → exit 0
- `npx eslint src/` → exit 0
- `npm test` → exit 0（重点：`bootstrap.test.ts` 3 例绿）
- `npx vite build` → exit 0
