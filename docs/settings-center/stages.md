# 设置中心（F11）Stage 划分

- 依据：`checklist.md`（20 项）+ `execution-plan.md`（编排参数）
- 原则：Stage 内文件不重叠；Stage 串行 + 每 Stage commit；文档同步固定末位 Stage
- **偏离规则豁免**：S01/S04/S05 仅 1–4 项（强耦合同文件，并行无收益，划分理由即同文件串行/单体小步）

## Stage 总览

| Stage | 名称 | 改动项 | agent 数 | 门禁 | 人工验证点 |
|-------|------|--------|----------|------|-----------|
| 01 | 后端 plan_balance 动态间隔 | SC-BE-01..04 | 1 | clippy + cargo test + rustfmt | 无（L4 兜底真实 invoke） |
| 02 | 设置中心框架 + 频率页 | SC-FE-01..04 | 2（pipeline） | tsc + eslint + npm test | 无 |
| 03 | hooks 迁入 + 原子切换 ⚠️ | SC-FE-05..07 | 3（并行） | tsc + eslint + npm test | 有（配置钮全链实测） |
| 04 | 快捷键页 | SC-FE-09 | 1 | tsc + eslint + npm test | 有（录制屏蔽实测） |
| 05 | 切项目自动关闭 | SC-FE-08 | 1 | tsc + eslint + npm test | 无 |
| 06 | E2E 完整覆盖 | SC-E2E-01..02 | 2（pipeline） | tsc + eslint + npm test + vite build + npm run e2e | 无 |
| 07 | 文档同步 | SC-DOC-01..05 | 2（并行） | 四级实跑取数 + md 转义残留 grep | 无 |

## Stage 01 后端 plan_balance 动态间隔（SC-BE-01..04）

**分工**：单 agent（mod.rs 同文件串行 BE-01/02/04 + BE-03 三处注册）

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| be | SC-BE-01..04 | src-tauri/src/plan_balance/mod.rs、src-tauri/src/settings.rs、src-tauri/src/lib.rs、src-tauri/build.rs、src-tauri/capabilities/default.json |

**实现要点**：原子量模块级（R2 修订，SNAPSHOT :66 先例）；poller 弃 ticker 改 loop{poll; sleep（内存值）}——interval period 不可变；命令顺序写死 校验→落盘→内存；save_settings 直接 await 复用（FB-05）；default.json :53 末位补逗号（FB-08）。

**验证项**（verify/stage-01.md 详表）：
1. `POLL_INTERVAL_SEC` grep ≥3 命中
2. 4 新用例 + 既有 plan_balance/settings 全绿
3. 三处注册 grep 各命中（lib.rs/build.rs/default.json）
4. `tokio::time::interval(` 在 mod.rs 零命中（语义式：须 Read 确认弃 ticker）
5. save_settings 复用（无第二写通道——语义式 Read 确认无 NamedTempFile 新建）

**commit**：`feat(plan_balance): 轮询间隔运行期可改 + plan_balance_set_interval 专用命令（F11）`

**人工验证点**：无（三处注册缺失的静默失败由 Stage 06 L4 频率用例真实 invoke 兜底）

## Stage 02 设置中心框架 + 频率页（SC-FE-01..04）

**分工**（pipeline 两相位，B 依赖 A 的类型定义）：

| label | 相位 | 负责项 | 触碰文件 |
|-------|------|--------|---------|
| A | 1 | SC-FE-01 注册表+类型、SC-FE-02 编排 | src/features/settingsCenter/{types.ts, SettingsPageRegistry.ts, index.ts, openSettings.ts}、src/workspace/pageApis.ts、src/__tests__/{settings-page-registry.test.ts, open-settings.test.ts, open-settings-panel.test.ts} |
| B | 2 | SC-FE-03 壳、SC-FE-04 频率页 | src/panels/settings/{SettingsPanel.tsx, index.ts, pages/PlanBalancePage.tsx}、src/features/settingsCenter/pages.ts、src/ipc/planBalance.ts、src/__tests__/{settings-panel.test.tsx, settings-plan-balance.test.tsx, ipc-plan-balance-contract.test.ts} |

文件零重叠（A=features/settingsCenter 四文件+pageApis+三测试；B=panels/settings+pages.ts+ipc+三测试）。

**中间态注意**：本 Stage 不注册 panelRegistry（PANEL_TYPES 仍 6 含 hooksConfig），配置钮未切换——verify 断言按此中间态。

**实现要点**：注册表家族契约 #13（惰性单例 + register/getAll/_reset）；壳是 params 持久化单点（FB-09 先例改 `settings-` 前缀）；side-effect import 触发点登记；onDidParametersChange 扁平事件结构红线。

**验证项**（verify/stage-02.md 详表）：
1. 6 新测试文件绿
2. panelRegistry.ts 未被触碰（grep hooksConfig 仍在——中间态断言）
3. ipc 契约 payload 键集合精确
4. pages.ts 仅注册 planBalance（order 20）

**commit**：`feat(settings): 设置中心框架（注册表/壳/打开编排）+ 套餐余量频率页（F11）`

**人工验证点**：无

## Stage 03 hooks 迁入 + 原子切换（SC-FE-05..07）⚠️ 高风险 Stage

**分工**（3 agent 并行）：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A | SC-FE-05 迁移部分 | git mv 编辑器 10 文件 + schema/ → cliProfiles/profiles/claude/configEditor/；claude/index.ts:16；删 panels/hooksConfig/{HooksConfigPanel.tsx,index.ts}；8+3 测试文件路径更新（FB-22） |
| B | SC-FE-05 HooksSettingsPage 部分 + SC-FE-07 | src/panels/settings/pages/HooksSettingsPage.tsx、src/features/settingsCenter/{pages.ts 追加, dirtyRegistry.ts}、src/panels/settings/SettingsPanel.tsx（dirty 汇聚）、src/workspace/PageDockviewHost.tsx:458、hooks-config-panel.test.tsx→settings-hooks-page.test.tsx、hooks-config-sync.test.tsx 改造、mock-cli-profile.test.tsx 改造、src/__tests__/{settings-panel-dirty.test.tsx, settings-dirty-registry.test.ts, workspace-defaulttab.test.tsx} |
| C | SC-FE-06 | src/panelRegistry.ts、src/features/sideViews/ActivityBar.tsx、src/workspace/pageApis.ts（删函数+:11 注释）、删 src/features/hooksConfig/openHooksConfig.ts、panel-registry.test.ts、layout-serde.test.ts、workspace-file-panel-types.test.ts、activityBar.test.tsx、删 open-hooks-config×2 |

文件零重叠：A=cliProfiles/**+panels/hooksConfig 删除+hooks-config 系 8 测试；B=panels/settings/**+settingsCenter/dirtyRegistry+PageDockviewHost+hub 语义三测试；C=panelRegistry/ActivityBar/pageApis/open* 测试。

**实现要点**：迁移 import 改造表照抄 checklist SC-FE-05 步骤 2（19 行逐行对应）；HooksSettingsPage 根容器保留 `data-e2e="hooks-config-panel"`；panelRegistry 原位替换保末位；isAlwaysRenderPanel 排除 settings（决策写死）；× 拦截 `panel.view.contentComponent`（非 `panel.component`）。

**验证项**（verify/stage-03.md 详表）：
1. `grep -rn "panels/hooksConfig" src/ src-tauri/ e2e-tests/` 零命中
2. `grep -rn "features/hooksConfig" src/` 零命中（注释同步后；eventsCatalog.ts:4 注释已改）
3. PANEL_TYPES 末位 settings 且 length 6
4. 旧 hooksConfig 布局白名单过滤新用例绿
5. × 拦截断言绿
6. PANEL_TYPES 中间态核对：本 Stage 后 = [terminal, editor, htmlviewer, gitshow, diff, settings]

**commit**：`feat(settings): hooks 配置迁入设置中心，hooksConfig 面板类型退役（F11）`

**人工验证点**：debug 包实测——配置钮打开设置中心 / hooks 页三层编辑保存 / 注入卸载 / 切 CLI dirty 守卫 / 旧布局 hooksConfig 面板丢弃不崩 / × 关闭 dirty 确认

## Stage 04 快捷键页（SC-FE-09）

**分工**：单 agent

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| fe-kb | SC-FE-09 | src/features/shortcuts/ShortcutRegistry.ts、src/panels/settings/pages/KeybindingsPage.tsx、src/features/settingsCenter/pages.ts（追加）、src/__tests__/{settings-keybindings.test.tsx, shortcuts.test.ts} |

**实现要点**：setCaptureSuspended/getEffectiveKeystroke 两 API 原文照抄 checklist；录制监听挂 window capture；纯修饰键按 code 判定；findConflict 页内纯函数导出；录制期间 suspended 防自触发（录 Ctrl+Shift+C 不真复制）。

**验证项**（verify/stage-04.md 详表）：两 API grep 存在；suspended 不消费断言绿；录制交互用例全绿；command-catalog 9 条不动。

**commit**：`feat(settings): 快捷键可视化配置页（F11）`

**人工验证点**：实测录制 Ctrl+Shift+C 不触发复制、改绑立即生效、保留键行内拒绝、Esc 取消

## Stage 05 切项目自动关闭（SC-FE-08）

**分工**：单 agent

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| fe-ac | SC-FE-08 | src/panels/settings/SettingsPanel.tsx、src/__tests__/settings-panel-autoclose.test.tsx |

**实现要点**：activePageId null 不动（防连锁误关）；初始不一致静默关（布局恢复场景）；dirty confirm 取消则不关。

**验证项**（verify/stage-05.md 详表）：5 用例绿（含 activePageId null 不关、初始不一致静默关）。

**commit**：`feat(settings): 切项目自动关闭设置中心面板（F11）`

**人工验证点**：无

## Stage 06 E2E 完整覆盖（SC-E2E-01..02）

**分工**（pipeline 两相位，B 依赖 A 的 helper）：

| label | 相位 | 负责项 | 触碰文件 |
|-------|------|--------|---------|
| A | 1 | SC-E2E-01 | e2e-tests/helpers.ts、src/__tests__/app.test.tsx |
| B | 2 | SC-E2E-02 | e2e-tests/settings.e2e.ts（新建）、e2e-tests/hooks.e2e.ts、e2e-tests/mockcli.e2e.ts |

**实现要点**：helpers.ts 在根 tsconfig include 外——本 Stage 门禁补 `npx vite build` 打包图验证；hooks/mockcli 适配点 = addPanel component/params + 清理分支 component 字面量（FB-19），选择器两处均不变（FB-18/SC-FE-05 决策）；L4 必须 VITE_E2E=1 构建（用 npm run e2e）。

**验证项**（verify/stage-06.md 详表）：settings.e2e 10 用例绿；hooks.e2e/mockcli.e2e 适配后绿；L4 计数 40→50；频率用例真实 invoke 兜底三处注册（SC-BE-03 盲区闭环）。

**commit**：`test(e2e): 设置中心 L4 覆盖 + hooks 用例适配（F11）`

**人工验证点**：无

## Stage 07 文档同步（SC-DOC-01..05）

**分工**（2 agent 并行）：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A | SC-DOC-01/02/03 | docs/settings-center-requirements.md、CONTEXT.md、.claude/CLAUDE.md、.claude/adr.md |
| B | SC-DOC-04/05 | src/panels/CLAUDE.md、src/features/settingsCenter/CLAUDE.md（新建）、删 src/features/hooksConfig/CLAUDE.md、src/features/cliProfiles/CLAUDE.md、src/features/shortcuts/CLAUDE.md、src/features/sideViews/CLAUDE.md、src/ipc/CLAUDE.md、src-tauri/src/CLAUDE.md、src-tauri/src/plan_balance/CLAUDE.md、src/workspace/CLAUDE.md、src/__tests__/CLAUDE.md、.claude/test-inventory.md |

文件零重叠（A=根级文档；B=模块文档+清单）。

**实现要点**：ADR-0011 代码自证原则（只记 why/红线/登记）；KZ-1 重写（编辑器归域后不再跨 panels）；test-inventory 三处计数一致（表头/段头/段小计）实跑取数。

**验证项**（verify/stage-07.md 详表）：R1–R3 回写命中；F11 登记根表；ADR-0012 存在；inventory 计数一致；md 无反斜杠紧跟反引号的转义残留（grep 该两字符序列零命中）。

**commit**：`docs(settings): F11 文档同步——术语/ADR-0012/模块文档/用例清单`

**人工验证点**：无

## 收尾人工实测汇总（全 Stage 完成后一次性执行）

1. Stage 03：配置钮全链（打开/hooks 三层编辑/注入卸载/切 CLI dirty/旧布局丢弃/× 确认）
2. Stage 04：录制屏蔽（Ctrl+Shift+C 不复制/改绑生效/保留键拒绝）
3. 频率页：改 120 → 观察余量刷新节奏变化（动态间隔生效实测）
