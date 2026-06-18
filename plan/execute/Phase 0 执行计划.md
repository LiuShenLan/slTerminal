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
