# Stage 02 逐项验证断言（唯一真值源）

> stage-02-frontend.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态：本 Stage 后 test-inventory 未登记（Stage 03 才登记）——不断言 test-inventory 内容；navTree/ipc CLAUDE.md 未更新——不断言文档内容。

## 断言清单

### PB-FE-01 DTO

- **PB-FE-01-a**：src/types/planBalance.ts 存在；grep `export interface PlanBalanceInfo` / `AmountInfo` / `WindowsInfo` / `WindowInfo` 四处命中
- **PB-FE-01-b**：Read 确认 PlanBalanceInfo 键集合精确 = sourceId/planId/frozen/amount/windows/updatedAt（camelCase，与后端 serde 双边对应）；WindowInfo 的 remainingPercent 为 number、resetsAt 为 `string | null`

### PB-FE-02 IPC

- **PB-FE-02-a**：src/ipc/planBalance.ts 存在；grep `get_plan_balance` / `refresh_plan_balance` / `plan-balance-updated` 命中
- **PB-FE-02-b**：Read 确认 onPlanBalanceUpdated 返回 unsubscribe 函数（listen 解包 event.payload 形态，notify.ts 先例）
- **PB-FE-02-c**：grep `export \* as planBalance` 命中 src/ipc/index.ts
- **PB-FE-02-d**：invoke 不出现场域外——grep `from "@tauri-apps/api/core"` 于 src/features/navTree/planBalanceModel.ts、usePlanBalance.ts、PlanBalanceFooter.tsx 零命中（硬约束 #1）

### PB-FE-03 全局 mock 双登记

- **PB-FE-03-a**：grep `../ipc/planBalance` 命中 src/__tests__/setup.ts；Read 确认 mock 三函数形态（getPlanBalance/refreshPlanBalance resolve 空数组；onPlanBalanceUpdated 返回 no-op 取消函数）
- **PB-FE-03-b**：grep `ipc/planBalance` 命中 src/__tests__/CLAUDE.md（全局 mock 策略节第四条登记，TQ-CI-05）

### PB-FE-04 契约测试

- **PB-FE-04-a**：src/__tests__/ipc-plan-balance-contract.test.ts 存在；grep `describeIpcContract` / `get_plan_balance` / `refresh_plan_balance` / `plan-balance-updated` 命中
- **PB-FE-04-b**：Read 确认含 DTO 键集合断言（Object.keys 精确匹配六键）；npm test 输出中该文件用例全绿（据测试 agent 结果判定）

### PB-FE-05 纯函数

- **PB-FE-05-a**：src/features/navTree/planBalanceModel.ts 存在；grep `export function currencySymbol` / `planLogoSrc` / `formatResetTime` / `formatUpdatedAt` / `rowText` / `rowTooltip` 六处命中
- **PB-FE-05-b**：Read formatResetTime 确认 D12 语义：<1h `Xm 后重置`、<24h `Xh Ym 后重置`（m=0 省略 m 段）、≥24h `M月d日 HH:mm 重置`（月日无前导零）、diff≤0 clamp `0m 后重置`、缺失/解析失败 null
- **PB-FE-05-c**：grep `theme` 于 planBalanceModel.ts 零命中（纯函数零依赖 theme）
- **PB-FE-05-d**：src/__tests__/plan-balance-model.test.ts 存在且 npm test 全绿（据测试 agent 结果判定）

### PB-FE-06 hook + 组件 + 挂载

- **PB-FE-06-a**：src/features/navTree/usePlanBalance.ts 存在；grep `REFRESH_THROTTLE_MS` 命中且值为 5000；Read 确认节流在 hook 内（lastRefreshRef 时间戳比较，D7）+ 双 catch 补 console.error
- **PB-FE-06-b**：src/features/navTree/PlanBalanceFooter.tsx 存在；grep `data-e2e="plan-balance-footer"` / `data-e2e="plan-balance-row"` 命中；Read 确认 items.length===0 → return null（整块含发丝线不渲染，§8.3）；grep `onError` 命中（logo 缺失隐藏兜底）
- **PB-FE-06-c**：**位置断言（U1，语义式）**——Read src/features/navTree/NavTree.tsx 渲染段确认 `<PlanBalanceFooter />` 位于树滚动区 div 之后、「添加项目」钮之前（不在侧栏最底部、不在树区内部）
- **PB-FE-06-d**：**颜色断言（语义式，须 Read 确认）**——PlanBalanceFooter.tsx 中不存在任何硬编码颜色值（不限写法：`#xxx`/`rgb(...)`/`rgba(...)`/颜色名字面量均属违规；`rgba(255,255,255,0.055)` 也不接受——必须经 SEPARATOR_BG token）；颜色引用仅限 DIM_FG/SEPARATOR_BG 两个 token
- **PB-FE-06-e**：src/__tests__/plan-balance-footer.test.tsx 存在且 npm test 全绿（据测试 agent 结果判定）

### 回归（语义式）

- **REG-NAVTREE**：`git diff --stat HEAD` 输出中**不含**任何既有 nav-tree 测试文件（nav-tree\*.test.tsx）——既有用例零改动（全局 mock 返回 [] → footer 默认隐藏）
- **REG-ICONS**：grep `plan-icons` 命中 PlanBalanceFooter 或 planBalanceModel（logo 路径约定 /plan-icons/<planId>.png）；`public/plan-icons/deepseek.png` 与 `kimi.png` 存在（Glob 命中，U3）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
