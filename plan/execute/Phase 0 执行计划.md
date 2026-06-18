# Phase 0 执行计划（`/goal` 驱动 + 多 agent 编排）

> 本文是 Phase 0「工程与测试基建」的**执行 runbook**，落地依据《[5.0 phase-0-工程与测试基建](../5.0 phase-0-工程与测试基建.md)》、方法学依据《[4. 完整开发计划](../4. 完整开发计划.md)》。
> 编排策略：**`/goal` 为主干**（锁 DoD、循环至全绿），**dynamic workflow 为旁路**（仅做少 agent 的版本核验与验收审计），**主会话顺序 spawn 子 agent** 推进有依赖的 bootstrap。
> 仅供审阅，未执行。

---

## 1. 编排总览

### 1.1 为什么这么编排

Phase 0 本质是**有强依赖的串行 bootstrap**：脚手架 → 后端骨架 → 依赖 → 测试基建 → CI → 守卫。后一步依赖前一步的产物，宽度并行收益有限，且多 agent 并发改盘易互相冲突。因此：

- **主干用 `/goal`**：Phase 0 有清晰可测的终态（DoD 全绿），天然适配 `/goal` 的「设条件→循环至达成」。
- **宽度并行只用在两处收益明确的旁路**，交给 dynamic workflow：**W0 前置版本/坑核验**、**W1 收尾验收审计**；均少 agent、只读核验。
- **串行 bootstrap 本身不进 workflow**（强依赖、需顺序、改文件互相影响）——这正是选 `/goal` 为主的原因。

### 1.2 三层结构

```
外层  /goal ............. 锁 Phase 0 DoD，每轮自动判定，未达成则续轮，达成自动 clear
  中层  主会话顺序编排 ... A1 脚手架 → A2 后端骨架 → A3 依赖 → A4 测试基建(L1–L4) → A5 CI → A6 守卫
  旁路  dynamic workflow . W0 版本/坑核验(前置, 可选) ; W1 DoD 验收审计(收尾, 可选)
```

### 1.3 版本前提与降级

| 能力 | 最低版本 | 自检 / 开启 |
|------|---------|------------|
| `/goal` | Claude Code 2.1.139+ | `claude --version` |
| dynamic workflow | 2.1.154+（Max/Team/Enterprise/API 默认开；Pro 在 `/config` 的 Dynamic workflows 行开） | `claude --version` + `/config` |

**降级**：若 `/goal` 不可用 → 改为人工逐项验收的顺序推进（§4 各 agent 验收仍成立）；若 workflow 不可用 → W0/W1 改为单会话内联调研/审计（本就可选）。两者缺失不阻塞 Phase 0 主体。

---

## 2. 如何使用 `/goal`（主干）

### 2.1 机制要点（决定怎么写条件）

- `/goal` 是**会话级、基于 prompt 的 Stop hook**：每轮结束，把「完成条件 + 至今对话」发给小快模型（默认 Haiku），返回 yes/no + 理由；未达成自动开下一轮，达成或 `/goal clear` 才停。
- **评判只读对话里 Claude 已暴露的产出**，它**不自己跑命令、不读文件**。⇒ **凡声称「通过」的项，必须把真实命令输出贴回主会话**，否则评判失明（漏判未完成、或被一句空话误导判完成）。这条贯穿 §4 所有 agent。
- 单会话仅一个 goal；条件可达 4000 字；可加「or stop after N turns」封顶；`/goal` 无参看状态（轮数/token/最近理由），`/goal clear`（或 stop/off/reset/cancel）清除；`--resume` 恢复未达成的 goal。

### 2.2 设定完成条件（直接可用）

把 DoD 写成**可由命令输出自证**的清单。CI 与本机 e2e 依赖网络/驱动，**不放进** `/goal` 条件（评判看不到 GitHub），改由 §5 单独验。设定：

```
/goal Phase 0 工程与测试基建完成，且下列每项均有对应命令的真实输出在本会话中自证：
(1) 在 src-tauri 下 `cargo test` 退出码 0，输出含 AppError 序列化样例用例 PASS；
(2) 前端 `npm test`（Vitest）退出码 0，输出含 mockIPC 样例（mock ping）PASS；
(3) L3 终端样例：经 @xterm/headless + @xterm/addon-serialize，feed "hi" 后 serialize 结果含 "hi"，断言 PASS；
(4) `tauri build --debug` 成功并产出 exe（贴出产物路径）；
(5) ESLint 守卫生效：在 src/ipc/ 之外写一处 @tauri-apps/api/core 的 invoke 会 lint 报错（贴报错），随后删除该反例、lint 恢复绿；
(6) 目录骨架齐全且可编译：前端 ipc/ types/ stores/ workspace/ panels/ features/ theme/ lib/ 与后端 pty/ fs/ git/ claude/ notify/ 各有占位，`cargo build` 与 `npm run build` 均通过。
硬约束：不得为通过校验而删改/弱化任何样例断言，不得放宽 lint 规则，不得引入任何业务功能。
以上未全部满足则继续；或在 40 turns 后停止并逐项汇报缺口。
```

> 注意「评判只优化所测项」：上面把「不得弱化断言/不得放宽 lint/不得引业务」写进硬约束，防止为绿而作弊式通过。

### 2.3 运行方式

- 交互式：先 `/goal …` 设定，再按 §4 顺序推进；`/goal` 随时查进度。
- 非交互：`claude -p "/goal …（同上）"` 一次跑完循环，Ctrl+C 可中断。
- L4(e2e) 与 CI 不在条件内，作为 §5 的独立门处理。

---

## 3. 如何用 dynamic workflow 编排多 agent（旁路，少 agent）

### 3.1 使用方式与边界

dynamic workflow 的脚本由 **Claude 自动编写**、运行时后台执行多 subagent，中间结果留在脚本变量、只把最终结果回灌上下文。你做的是：**描述任务 + 触发 → 审批计划 → `/workflows` 观察 → 满意则 `s` 存为命令**。关键约束：

- 触发：prompt 含 `ultracode` 关键字 / 自然语言「用 workflow」/ `/effort ultracode`（整会话自动编排）/ 已存命令 / `/deep-research`。
- 子 agent **强制 `acceptEdits`、继承你的 tool allowlist**；脚本本身不碰 FS/shell，只协调 agent。
- 并发 ≤ 16、单次 ≤ 1000 agent；退出 CC 即重来（跨会话不续）→ **单会话内跑完**。
- 不在 allowlist 的 shell/web/MCP 仍会中途弹权限 → **预先把 `cargo`/`npm`/`gh` 等加 allowlist** 再跑长任务。

Phase 0 只在下面两处用它（均「资料核验/验证」性质，少 agent）。

### 3.2 W0 — 版本与坑核验（前置，可选）

- **目的**：把 5.0 里要钉的版本与 Windows 坑一次核清，产出「版本钉表 + 坑清单」喂给 A1/A3/A4。
- **触发**（任一）：

  ```
  ultracode: 核验 slTerminal Phase 0 技术栈的当前版本钉与 Windows 坑，分头检索后交叉核验，产出版本钉表+坑清单
  ```

  或直接 `/deep-research <同一问题>`。
- **扇出角度**（每 agent 一题，互相 cross-check）：① Tauri 2 当前稳定版与 `create-tauri-app`(React+TS+Vite) 模板现状；② `@xterm/xterm`/`addon-webgl`/`addon-fit`/`headless`/`addon-serialize` 版本与兼容；③ `tauri-driver`+WebdriverIO+`msedgedriver` 与 WebView2 版本对齐法；④ Dockview 暗色集成注意点；⑤ `@tauri-apps/api/mocks` `mockIPC` 现行 API。
- **synthesize**：汇成版本钉表（含每条来源、经交叉核验）回灌主会话。
- **验收该 workflow**：每条 claim 有来源且交叉核验通过；版本钉表可直接落 `package.json`/`Cargo.toml`。满意则 `s` 存为 `/verify-stack`（后续 phase 复用）。

### 3.3 W1 — DoD 验收审计（收尾，可选）

- **目的**：在 `/goal` 收口前，用独立 agent 交叉验 DoD，产出「DoD×证据」表回灌主会话，正好作为 `/goal` 评判可见的证据。
- **触发**：

  ```
  ultracode: 审计 slTerminal Phase 0 的 DoD 各项；每项独立跑命令并贴真实输出；只验证不修改源码与断言
  ```
- **扇出**：6 个 agent 各审一项 DoD（cargo test / Vitest / L3 / build / e2e / invoke-lint+骨架），各自独立跑命令贴输出；外加 1 个对抗审查 agent，专查「是否有为过测而弱化断言/放宽 lint」。
- **synthesize**：一张 DoD×证据对照表 + 红旗清单回灌主会话。
- **强约束**：子 agent 是 `acceptEdits`、会自动改盘 → prompt 里明令**只读验证、不改源、不改断言**。
- **验收该 workflow**：对照表覆盖 6 项 DoD、证据为真实命令输出、红旗清单为空。

### 3.4 何时**不**用 workflow

A1–A6 的串行 bootstrap 不要塞进 workflow（强依赖、需顺序、并发改盘冲突）。主干始终是 `/goal` + 主会话顺序 spawn 子 agent。

---

## 4. 各 agent 工作内容与验收（中层顺序编排）

主会话作为 orchestrator，用 Agent 工具按依赖顺序 spawn 下列子 agent；**每个 agent 必须把验证命令的真实输出回灌主会话**（喂 `/goal` 评判）。依赖：A1→A2→A3→A4→A5；A6 可与 A4 并行；W0 在 A1 前、W1 在 A5 后。

> 每个 agent 的 prompt 统一附：① 本 agent 任务卡（下表）；② 架构第七节相关硬约束；③「只做本 agent 范围、不越界改他人产物」；④「跑完把命令输出原样贴回」。

| Agent | 任务（做什么） | 产出 | 验收（命令级，须贴输出） | 建议模型 |
|------|----------------|------|--------------------------|---------|
| **A1 脚手架** | `npm create tauri-app`(React+TS+Vite) 初始化；窗口暗色；`App.tsx` 渲染填满窗口的暗色空 `DockviewReact`；铺前端 8 空目录(`ipc/ types/ stores/ workspace/ panels/ features/ theme/ lib/`)各占位 index | 可运行空壳前端 + 暗色空 Dockview | `npm run tauri dev` 起暗色空窗、无报错；8 目录存在 | sonnet |
| **A2 后端骨架** | `main.rs` 仅 `app_lib::run()`；`lib.rs` 的 `generate_handler!`(暂放 `ping`)；`error.rs` 的 `AppError`(thiserror+Serialize)；`state.rs` 的 `AppState` 空壳并 `manage`；接 `tracing`；后端 5 空模块(`pty/ fs/ git/ claude/ notify/`)各 `mod.rs` 占位 | 可编译后端骨架 | `cargo build` 通过；`cargo test` 含 AppError 序列化样例 PASS | sonnet |
| **A3 依赖** | 前端装 `dockview @xterm/xterm @xterm/addon-webgl @xterm/addon-fit @xterm/headless @xterm/addon-serialize`(termless 0.x 可选) `vitest @tauri-apps/api`；后端 `portable-pty git2 notify`(占位)。版本依 **W0 钉表** | 锁定的 lockfile + Cargo.lock | `npm run build` 与 `cargo build` 均通过；装的版本与钉表一致 | sonnet |
| **A4 测试基建** | 四层各一可运行样例（可内部再扇出 L1–L4 四子 agent）：**L1** `cargo test`(AppError 序列化)；**L2** Vitest+`@tauri-apps/api/mocks` 的 `mockIPC`(mock `ping`)；**L3** `@xterm/headless`+serialize(feed 文本→断言)；**L4** `e2e-tests/` 配 `tauri-driver`+WebdriverIO(Windows `msedgedriver`)，样例：启动应用断言窗口标题 | 四层样例 + 配置 | L1/L2/L3 各样例 PASS；`tauri build --debug` 成功；L4 样例 PASS（本机有 `msedgedriver` 时，否则交 CI） | opus |
| **A5 CI** | `.github/workflows/ci.yml`：`windows-latest` 跑 `cargo test`+`npm test`+L3+`tauri build --debug`+关键 L4；钉工具链版本、`msedgedriver`↔WebView2 同步 | CI 配置 | push 分支后 `gh run watch` 全绿（见 §5.2） | opus |
| **A6 守卫** | ESLint 规则：禁止 `@tauri-apps/api/core` 的 `invoke` 出现在 `src/ipc/` 之外；文档注明「命令返回 `Result<_,AppError>`」「阻塞 I/O 用 `spawn_blocking`」**无法 lint**、靠约定+review | lint 规则 + 说明 | 在 ipc 外写一处 `invoke`→lint 报错（贴）→删反例→lint 绿 | sonnet |

---

## 5. 整个 Phase 0 如何验收（DoD 闭环）

### 5.1 自动化（机器自证，`/goal` 评判可见）

即 §2.2 条件 (1)–(6)。每项对照命令与期望：

| DoD 项 | 命令 | 期望 |
|--------|------|------|
| 后端单测 | `cargo test`（src-tauri/） | 退出 0，含 AppError 序列化样例 PASS |
| 前端单测 | `npm test` | 退出 0，含 mockIPC(ping) 样例 PASS |
| L3 终端 | 跑 L3 样例 | feed "hi" → serialize 含 "hi" |
| 构建 | `tauri build --debug` | 成功产出 exe |
| 守卫 | ipc 外 invoke 反例 → lint | 报错；删反例后绿 |
| 骨架 | `cargo build` + `npm run build` | 均通过，前后端目录齐全 |

### 5.2 CI 门（独立于 `/goal`，需推送）

`/goal` 评判看不到 GitHub。单独走：push 分支 → `gh run watch`（或 `gh run view`）把结果**输出回会话** → 确认 `windows-latest` 全绿。这是 Phase 0 DoD 的「CI 绿」项。

### 5.3 L4(e2e) 本机门（条件性）

本机 `msedgedriver` 与 WebView2 对齐时，本地跑 L4 样例 PASS；否则以 §5.2 的 CI L4 为准。

### 5.4 人工（机器够不到）

应用启动呈现**暗色空 Dockview 窗口、无报错**——肉眼勾选。

### 5.5 完成定义（DoD，对齐 5.0 §8）

骨架目录 + 四层测试各有可运行样例 + CI 绿 + `invoke` 守卫生效。`/goal` 条件被判定满足并自动 clear ⇒ 5.1 达成；再过 5.2/5.3/5.4 ⇒ Phase 0 收口（此阶段无既有回归集，本阶段即建立基线）。

---

## 6. 执行顺序速查

1. `claude --version` 自检（`/goal`≥2.1.139、workflow≥2.1.154）；Pro 在 `/config` 开 workflow；把 `cargo`/`npm`/`gh` 加进 allowlist。
2. （可选）跑 **W0** `/verify-stack` 或 `ultracode: …` → 得版本钉表（喂 A1/A3/A4）。
3. `/goal <§2.2 条件>` 设定外层目标。
4. 主会话顺序 spawn **A1→A2→A3→A4(含 L1–L4)→A5**，**A6 并入**；每个 agent 把验证输出回灌。
5. （可选）跑 **W1** 验收审计 workflow → DoD×证据表回灌主会话。
6. `/goal` 判定达成自动 clear；push 分支 → `gh run watch` 确认 CI 绿（§5.2）。
7. 人工勾选暗色空窗（§5.4）。

> **降级**：`/goal` 缺失 → 纯顺序 spawn + 手动逐项验收；workflow 缺失 → W0/W1 内联做。均不阻塞主体。

---

## 7. 注意事项（踩坑前置）

- **证据回灌是 `/goal` 正确的命门**：每步必须把真实命令输出贴回主会话，否则评判失明 → 误判。
- **workflow 子 agent 会自动改盘**（`acceptEdits`）：W1 审计务必约束「只读验证、不改源/不改断言」；不在 allowlist 的命令会中途弹权限 → 预先放行。
- **workflow 不跨会话续**：单会话内跑完；大任务先小切片估费（`/workflows` 看 token，可随时 `x` 停且不丢已完成）。
- **e2e 绿的前提是 `msedgedriver`↔WebView2 对齐**（5.0 坑）；CI 与本地工具链版本须一致。
- **条件只测易测项会被钻空子**：§2.2 已把「不弱化断言/不放宽 lint/不引业务」写进硬约束，勿删。
- **Playwright 在 Tauri 不可用**（非 Chromium），E2E 只走 WebDriver；E2E 脆、只覆盖关键路径，主力在 L1–L3。

---

## 8. Phase 0 补救执行计划（`/goal` + dynamic workflow）

> 背景：Phase 0 首次执行后 L1–L3 与骨架真实通过，但经[验收审计](./Phase 0 验收审计.md)，L4 未接线、CI 未绿、4 处报告不实 + 6 处缺漏。本章针对 D1–D7 决策台账制定补救 runbook，使用 `/goal` + dynamic workflow 编排。**本文与 §1–§7 互补，仅覆盖补救项，已通过项(L1/L2/L3/骨架/lint)仅做回归验证。**

### 8.1 为什么补救用 dynamic workflow 为主（不同于首次的 `/goal` 为主）

首次 Phase 0 是**强依赖串行 bootstrap**(脚手架→骨架→依赖→测试→CI)，多 agent 并发改盘冲突。补救项 D1–D5+D7 是**独立文件编辑**——`error.rs`/`ci.yml`/`version-pins.md`/`Cargo.toml`/`lib.rs`/`package.json`/`wdio.conf.ts`/目录搬迁，互不冲突（唯一依赖是 D7 搬迁后 D1 才知 spec 路径）。因此：

- **1 个 dynamic workflow 全包**(D7 作为 Phase 1 屏障 → D1–D5 并行扇出 → 回归验证 → push CI)。
- **`/goal` 锁全 DoD 终态**，评判 workflow 回灌的命令输出。
- 不再需要首次那种"主会话顺序 spawn 6 个 agent"的中间层。

### 8.2 三层结构

```
/goal ................. 补救 DoD 条件（全 6 项 + 回归不破 + CI 绿），未达成自动续轮
  └ dynamic workflow .. 一次性后台编排：
       Phase 'Layout'   → agent D7 (目录迁移)
       Phase 'Fixes'    → parallel(agent D1, D2, D3, D4, D5)
       Phase 'Verify'   → parallel(cargo test, npm test, tauri build, npm run wdio)
       Phase 'CI'       → push → gh run watch → 回灌绿
```

### 8.3 `/goal` 完成条件

```
/goal Phase 0 补救全部完成，下列每项在本会话中均有真实命令输出自证：
(1) D7 目录布局对齐 5.0 §3：tree 输出显示 e2e-tests/(含 wdio.conf.ts+spec)、test/terminal/(含 L3)、src-tauri/tests/(含集成测试)、src/__tests__/ 已清空；
(2) D2 CI 警告策略已恢复：cargo build 无 error(含 -D warnings 生效)、AppError 有作用域 #[allow(dead_code)] 且注释"Phase 0 占位"；
(3) D4 tracing 已接入：cargo build 通过、输出含 tracing subscriber init 启动日志；
(4) D5 Tauri 版本互锁：package.json 和 Cargo.toml 两端版本皆精确钉(去 caret)且同一 patch，cargo build 与 npm run build 均通过；
(5) D3 notify 钉表已同步：version-pins.md 钉表 notify 行改为 9.0.0-rc.4、红旗 #8 标注"已评估可接受"；
(6) D1 L4 已接线并跑通：npm run wdio 退出 0，spec 输出含 getTitle 断言 PASS；
(7) 回归不破：cargo test 通过、npm test 通过(L2+L3)、tauri build --debug 成功；
(8) CI 真绿：git push 后 gh run view 输出全绿(含 L4 step，无 continue-on-error 掩盖)。
硬约束：不弱化断言、不放宽 lint、不改动测试逻辑(仅迁移路径)、不引入业务功能。
以上未全部满足则继续；40 turns 后停止并逐项汇报缺口。
```

### 8.4 Dynamic workflow 编排

#### 触发方式

```
ultracode: 按以下编排执行 slTerminal Phase 0 补救 D1-D7，只做计划内修改、不做无关改动，每阶段完成后贴命令输出回主会话
```

或设 `/effort ultracode`(整会话自动编排)后直接描述任务。推荐前者——单次触发、可控。

#### Workflow 脚本结构

```
Phase 'Layout' [barrier]
  agent D7 — 目录迁移，产出 tree 输出

Phase 'Fixes' [parallel]  ← D7 确认后并行扇出
  agent D1 — L4 接线(embedded driver)
  agent D2 — CI 警告策略 + AppError allow
  agent D3 — 钉表同步(notify + 红旗)
  agent D4 — tracing 接入
  agent D5 — Tauri 版本互锁(精确钉)

Phase 'Verify' [parallel]  ← D1–D5 全部完成后回归
  agent V1 — cargo test + cargo build(含 -D warnings)
  agent V2 — npm test(L2+L3)
  agent V3 — tauri build --debug
  agent V4 — npm run wdio(L4 本机)

Phase 'CI'
  agent push — git add + commit + push → gh run watch → 贴 CI 绿
```

#### 各 agent 任务卡

##### Agent D7 — 目录迁移(Phase 'Layout'，sonnet)

| 项目 | 内容 |
|------|------|
| **任务** | 将测试目录从当前布局迁移到 5.0 §3 指定布局 |
| **操作** | ① 建 `e2e-tests/` 目录，移入 `wdio.conf.ts` + `test/specs/test.e2e.ts`(重命名为 `test.e2e.ts`)；② 建 `test/terminal/` 目录，移入 `src/__tests__/terminal-serialize.test.ts`；③ 建 `src-tauri/tests/` 目录，创建第一个集成测试样板(验证 `lib` crate 可链接,1 个 `#[test]`)；④ L2 mockIPC 测试保留在 `src/__tests__/ipc-ping.test.ts`(5.0 `src/**/*.test.ts` glob 覆盖)；⑤ 删空的 `test/specs/` 目录；⑥ 更新 `package.json` 的 test/wdio scripts 适配新路径；⑦ 更新 `vitest.config.ts`(如有)或用默认 include(文件名 `*.test.ts` 天然被覆盖)；⑧ `tsconfig.json` 的 `include` 加 `"test/**/*"` |
| **产出** | `tree` 输出显示 `e2e-tests/wdio.conf.ts`、`e2e-tests/test.e2e.ts`、`test/terminal/terminal-serialize.test.ts`、`src-tauri/tests/integration_test.rs` 存在 |
| **验收** | 目录结构匹配 5.0 §3；`cargo test`(含新集成测试)、`npm test`(vitest 自动发现 test/ 下 `*.test.ts`)通过 |

##### Agent D1 — L4 接线 embedded driver(Phase 'Fixes'，opus)

| 项目 | 内容 |
|------|------|
| **任务** | 用 `@wdio/tauri-service` + `driverProvider: 'embedded'` 重写 `wdio.conf.ts`，跑通 L4 E2E spec |
| **操作** | ① `Cargo.toml` 加 `tauri-plugin-wdio-webdriver = "1"` 依赖；② `lib.rs` 注册 `.plugin(tauri_plugin_wdio_webdriver::init())`；③ 重写 `e2e-tests/wdio.conf.ts`：`services: [['tauri', { driverProvider: 'embedded', appBinaryPath: '../src-tauri/target/debug/slterminal.exe' }]]`，`capabilities` 配 `browserName: 'tauri'`+`'tauri:options'`，`specs: ['./**/*.ts']`(或 `['./test.e2e.ts']`)，`maxInstances: 1`，`framework: 'mocha'`，`mochaOpts.timeout: 60000`；④ `onPrepare` 不需编译(已在 verify 阶段前完成)，也不需 `beforeSession`/`afterSession`(service 自动处理)；⑤ 确保 spec 文件(`test.e2e.ts`)的 import 正确(`@wdio/globals`) |
| **产出** | 可运行的 embedded driver E2E 配置 |
| **验收** | `npm run wdio`(在 `e2e-tests/` 或更新 scripts)退出 0，输出含 spec PASS(标题断言)；不再依赖 msedgedriver |
| **注意** | 若 embedded 遇阻塞 bug，switch 到 `driverProvider: 'official'` 但锁 WDIO 7.x(需降级)——Phase 0 只做最小可行。但从社区数据看 embedded v1.1.0 对简单 spec 应可用 |

##### Agent D2 — CI 警告策略 + AppError(Phase 'Fixes'，sonnet)

| 项目 | 内容 |
|------|------|
| **任务** | 恢复 CI `-D warnings` + 给 `AppError` 加作用域 `#[allow(dead_code)]` |
| **操作** | ① `ci.yml` 删 `rustflags: ""`(第 25 行)，恢复 `actions-rust-lang/setup-rust-toolchain@v1` 默认的 `-D warnings`；② `error.rs` 第 5 行(enum 上方)加 `#[allow(dead_code)]` + 注释 `// Phase 0 占位变体，后续 phase 构造；保留 -D warnings 全局生效` |
| **产出** | CI 恢复 warning-as-error 门控，仅 AppError 占位变体被 allow |
| **验收** | `cargo build` 零 error、零 warning(dead_code 被 allow 抑制)；`cargo test` 通过 |

##### Agent D3 — 钉表同步(Phase 'Fixes'，sonnet)

| 项目 | 内容 |
|------|------|
| **任务** | 将 `version-pins.md` 的 notify 行从 8.2.1 改为实际使用的 9.0.0-rc.4，红旗 #8 标注"已评估可接受" |
| **操作** | ① `version-pins.md` 钉表 notify 行："8.2.1"→"9.0.0-rc.4"，兼容性说明从"生产环境应钉 stable 8.x"→"Phase 0 评估可接受(rc.4)，正式发布后单独验证"；② 红旗 #8 表项追加"**(已评估:Phase 0 无文件监听需求，rc.4 仅占位)**"；③ 确认报告 §9 不再写"对齐"(此为审计报告的事，D3 只管钉表) |
| **产出** | 钉表与实际依赖一致 |
| **验收** | grep `version-pins.md`：notify 行 = 9.0.0-rc.4、红旗 #8 = 已评估 |

##### Agent D4 — tracing 接入(Phase 'Fixes'，sonnet)

| 项目 | 内容 |
|------|------|
| **任务** | 最小接入 tracing：加 `tracing-subscriber` 依赖，在 `run()` 顶部 init subscriber + 一句启动日志 |
| **操作** | ① `Cargo.toml`：加 `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`(在 `tracing` 依赖后)；② `lib.rs`：`run()` 函数内首个语句插 `tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();`；③ 紧接一句 `tracing::info!("slTerminal 启动");`；④ 文件顶部已有 `mod error;` 等声明，`tracing` 和 `tracing_subscriber` 通过 `use` 或全路径引用均可 |
| **产出** | L5 基建实际可用 |
| **验收** | `cargo build` 通过；`cargo run`(或 tauri dev)输出含 `slTerminal 启动` 日志行(贴输出) |
| **注意** | `windows_subsystem = "windows"` 在 release 模式会吞 stdout,但 Phase 0 全用 debug build 不受影响 |

##### Agent D5 — Tauri 版本互锁(Phase 'Fixes'，sonnet)

| 项目 | 内容 |
|------|------|
| **任务** | `package.json` 和 `Cargo.toml` 两端去 caret 精确钉 |
| **操作** | ① `package.json`：`@tauri-apps/api` 从 `"^2.11.1"`→`"2.11.1"`(去 caret)；`@tauri-apps/cli` 已是 `"2.11.2"` 精确钉,不动；② `Cargo.toml`:`tauri` 已是 `"2.11.3"` 精确,不动；③ 确认 `@tauri-apps/api` 2.11.1(JS)与 tauri 2.11.3(Rust)patch 差在允许范围内(core API 允许 patch 不同,major/minor 相同即可)；④ **不加**自定义 CI 版本检查 step——`tauri build --debug` 已内置检测(CLI≥2.8.0) |
| **产出** | 版本精确锁定 |
| **验收** | `npm run build` 通过；`cargo build` 通过；`tauri build --debug` 通过(内置检测无不匹配报错) |

#### Phase 'Verify' — 回归验证(4 agent 并行，sonnet)

| Agent | 命令 | 期望 |
|-------|------|------|
| **V1 后端** | `cargo test`(src-tauri/) + `cargo build` | 退出 0，零 error，无未被 allow 的 warning，集成测试 PASS，启动日志可见 |
| **V2 前端** | `npm test` | 退出 0，L2 mockIPC + L3 terminal-serialize 均 PASS |
| **V3 构建** | `npm run tauri build -- --debug --no-bundle` | 成功产出 exe，无版本不匹配报错 |
| **V4 L4** | `npm run wdio`(或 `cd e2e-tests && npx wdio run wdio.conf.ts`) | 退出 0，spec PASS(getTitle 断言)，贴完整输出 |

> 4 个 agent 各自独立跑命令并贴真实输出回 workflow。synthesize 后回灌主会话给 `/goal` 评判。

#### Phase 'CI' — push + gh run watch(1 agent，sonnet)

| 项目 | 内容 |
|------|------|
| **任务** | 提交所有修改、推送、等 CI 全绿、贴输出 |
| **操作** | ① `git add -A && git commit -m "Phase 0 补救: D1-D7 收口"`；② `git push origin main`；③ `gh run watch`(或 `gh run view --log`)等全绿(含 L4 step——**不用 `continue-on-error: true`**，确认真绿)；④ 贴 `gh run view` 输出 |
| **产出** | CI 全绿证据 |
| **验收** | `gh run view` 所有 step 绿色勾，L4 step 不含 continue-on-error |

### 8.5 手动收尾

1. **人工验证**：启动 `npm run tauri dev`——暗色空 Dockview 窗口无报错(如首次已验可跳过)。
2. **重写验证报告**：将《Phase 0 结果验证.md》按审计报告 §2 的不实之处逐条修正(CI 绿、L4 已接、tracing 已接入、notify 9.0.0-rc.4、CI 警告策略已恢复)并补充 D1–D7 落实证据。
3. **DoD 逐条复核**：对照 5.0 §7.1/§7.2/§8，确认 10 条全绿→Phase 0 正式收口。

### 8.6 执行顺序速查

1. `claude --version`(≥2.1.154 有 workflow)，确认 `/effort` 可用。
2. 把 `cargo`/`npm`/`gh`/`git` 加进 allowlist；关闭 `continue-on-error`(改 `ci.yml` D2 agent 已做)。
3. 设 `/goal <§8.3 条件>`。
4. 触发 workflow：`ultracode: §8.4 编排 D1-D7 补救...` → 审批 → `/workflows` 观察。
5. Workflow 四个 Phase 全部跑通、证据回灌。
6. `/goal` 判定达成自动 clear。
7. 人工暗色窗口勾选(§8.5)。
8. 重写验证报告 → DoD 复核 → Phase 0 收口。

### 8.7 决策台账（补救部分）

| # | 议题 | 决策 | 落点 |
|---|------|------|------|
| E1 | 编排结构 | `/goal` 锁全 DoD + 1 个 dynamic workflow 全包 D1–D7 | §8.2 |
| E2 | D7 屏障 | workflow Phase 'Layout' 先迁目录，确认后并行 D1–D5 | §8.4 |
| E3 | L4 方案 | `@wdio/tauri-service` + `driverProvider: 'embedded'` + `tauri-plugin-wdio-webdriver` | D1 agent |
| E4 | D5 简化 | 精确钉版本 + 靠 `tauri build` 内置检测，去自定义 CI 版本检查 | D5 agent |
| E5 | D2 策略 | 恢复 CI `-D warnings` + `AppError` 加作用域 `#[allow(dead_code)]` | D2 agent |
| E6 | tracing | 最小接入：`tracing-subscriber` + `fmt::init()` + 一句 `info!` | D4 agent |
| E7 | L4 运行 | 本机跑(embedded 无需 msedgedriver) | V4 agent |
| E8 | CI L4 | 去 `continue-on-error: true`，L4 必须真绿 | D2+CI agent |

> 补救 runbook 与首次执行计划(§1–§7)互补：首次负责 "从零建"，补救负责 "补齐缺口 + 回归不破"。共同目标是 5.0 §8 DoD 全部满足。
