# REVIEW-b：spec 一致性 + pebrel 一手事实核查

角度 B 产出。核查面：spec↔设计一致性（采纳点落实 / 不采纳点越界 / 越权砍项）+ 对 pebrel 源码（baseline `e537d528`）的一手事实核对 + 产品定位约束 + 行号引用禁令。跨篇类型/命名/阶段/缝合断链、DECISIONS 与正文冲突的系统性排查归角度 A，本文不重复其已报项（A-3 git2 双轨、A-4 DA1 表值、A-9 WindowState 回放、A-19 ProviderTestOutcome 设计侧矛盾等）。

计数类失实的共同根因：多处枚举变体数/词表条数/字段数以快照数字入文，本身违反快照数字禁令；统一修法为删计数、引符号名，数值钉测试。

## 🔴 阻塞

无。

## 🟡 应修

### B-1 spec 03 采纳点 6：`AiHookKind`「七态」失实

- 定位：`docs/pebrel-refactor/03-ai-cli-integration.md` 采纳点 6
- 问题：spec 写「归一成 `AiHookKind` 七态」，括号内只列 6 个变体名；pebrel 一手核实 `nebula_app/src/ai_hook/event.rs` 实为 **6 变体**（SessionStart / PromptSubmit / ToolComplete / TurnDone / NeedsAttention / SessionEnd）。
- 影响：spec 计数失实；设计 03 只列变体名未继承计数，未扩散。
- 修法：删「七态」计数或改「六态」。

### B-2 design 03 hook 小进程：`Outcome`「七态」失实

- 定位：`docs/pebrel-design/03-ai-cli-integration.md` hook 小进程关键类型节
- 问题：注「Outcome 七态写 SLTERM_HOOK_LOG」；pebrel 一手核实 `nebula_hook/src/main.rs` `Outcome` 实为 **6 变体**（含 RemoteOsc），按本篇砍 RemoteOsc 后应为 5。
- 影响：计数双重失实（对 pebrel 现状、对砍后终态均不符）。
- 修法：删「七态」计数。

### B-3 `HookInspection`「八字段」双侧失实

- 定位：`docs/pebrel-refactor/03-ai-cli-integration.md` 优化面 2 + 采纳点 21；`docs/pebrel-design/03-ai-cli-integration.md` 模块终态表 / 关键类型节 / 缝合点 4 / 测试点
- 问题：两侧均写「八字段」；pebrel 一手核实 `nebula_app/src/ai_hook/integrations.rs` `HookInspection` 实为 **7 字段**（config_path / available / installed / needs_repair / enabled / helper_missing / error）。
- 影响：spec 与设计同错，计数已扩散到设计篇 4 处。
- 修法：删计数或改「七字段」。

### B-4 agent_detection「21 厂商」双侧失实

- 定位：`docs/pebrel-refactor/03-ai-cli-integration.md` 优化面 4 + 采纳点 27；`docs/pebrel-design/03-ai-cli-integration.md` 模块终态表 / 照抄清单 / 测试点
- 问题：两侧均写「21 个厂商」；pebrel 一手核实 `nebula_app/src/ai_agents.rs` 注册表 + `agent_detection/` 目录实为 **20 个厂商 TOML + `_shared.toml` 兜底**。
- 影响：计数失实（20 厂商 = 21 个文件，疑由此误记）。
- 修法：删计数或改「20 厂商 + `_shared.toml`」。

### B-5 `is_dangerous` 词表「21 条」双侧失实

- 定位：`docs/pebrel-refactor/08-ai-assistants.md` 采纳点 29；`docs/pebrel-design/08-ai-assistants.md` 关键类型节 / 改造节 9 / 测试点
- 问题：两侧均写词表 21 条；pebrel 一手核实 `nebula_app/src/ai_assistant.rs` `PATTERNS` 实为 **20 条**。
- 影响：计数失实且扩散到设计篇 3 处。
- 修法：删计数或改「20 条」。

### B-6 spec 08 采纳点 31：`ProviderTestOutcome`「十态」失实

- 定位：`docs/pebrel-refactor/08-ai-assistants.md` 采纳点 31
- 问题：spec 写「ProviderTestOutcome 十态」；pebrel 一手核实实为 **17 变体**。（设计 08 篇内前后矛盾已由 review-a A-19 覆盖，此处仅补 spec 侧失实。）
- 影响：spec 计数失实，是设计侧矛盾的源头。
- 修法：删「十态」计数。

### B-7 design 08 照抄清单 #49：`scientific_corpus` pebrel 源路径失实

- 定位：`docs/pebrel-design/08-ai-assistants.md` 照抄拷贝清单 第 49 条
- 问题：源路径写 `ai_hook/local/scientific_corpus.rs`；pebrel 一手核实实际位于 `nebula_app/src/scientific_corpus.rs`（同篇模块终态表的目标路径正确，仅清单源路径错）。
- 影响：照抄执行时按错路径取源文件。
- 修法：改源路径为 `nebula_app/src/scientific_corpus.rs`。

### B-8 design 06：语言注册行数「10→2」失实且篇内矛盾

- 定位：`docs/pebrel-design/06-settings-i18n.md` 改造节两处（「注册行从 10 行裁到 2 行」「注册行 10→2」）
- 问题：pebrel 一手核实 `nebula_settings/src/language.rs` `languages!` 实为 **11 注册**（ZhCn/EnUs/ZhTw/FrFr/DeDe/EsEs/PtBr/ItIt/RuRu/JaJp/KoKr）；同篇开放问题 1 已正确写「11 语」。
- 影响：裁量基数失实 + 篇内自相矛盾。
- 修法：改「11→2」或删计数。

### B-9 WSLENV 合并：spec 内部矛盾被设计继承，两篇设计方向分裂

- 定位：`docs/pebrel-refactor/03-ai-cli-integration.md` 采纳点 37（照抄 WSLENV 合并）vs `docs/pebrel-refactor/04-runtime-api.md` 不采纳点 6（明砍「按值判重的 WSLENV 合并」）vs `docs/pebrel-refactor/SPEC.md` 不采纳总表（砍「WSL 全家」）；设计侧 `03-ai-cli-integration.md` 保留（agent_env apply / 幂等三件套 / `wslenv_entries_match_variables` 测试）vs `04-runtime-api.md` 裁剪不迁（「agent_env.rs 的 WSL 臂」）
- 问题：spec 03 与 spec 04 对同一机制一采纳一砍（总表口径倾向砍），spec 内部矛盾未经裁决即分别落入两篇设计，03 保留、04 砍除，方向不一致。
- 影响：M4 实现期两篇设计给出相反指令；agent_env 幂等三件套与对应测试点存废悬空。
- 修法：提请用户裁决归口（建议按总表砍）；若砍，design 03 删 WSLENV 臂及 `wslenv_entries_match_variables` 测试点，若留，design 04 不迁清单删该条。

### B-10 design 08 改造节 3：虚构 spec 引文 + Posix 开放问题滞后于 B.21

- 定位：`docs/pebrel-design/08-ai-assistants.md` 改造节 3 + 开放问题 1
- 问题：改造节 3 引「spec 分片 08 不采纳九种额外 CLI 语言的一等支持」——spec 08 不采纳点无此条（「9 种额外语言」砍的是 i18n UI 语言目录，归总表/06 语境，与补全方言无关）；且开放问题 1 仍议「Posix 方言去留」，与 DECISIONS B.21「08开放1 Posix 方言 → 保留（Git Bash/MSYS 场景）」冲突，行文滞后于裁决。
- 影响：改造节 3 的存在依据失实；已裁事项仍挂开放问题，误导后续评审。
- 修法：改造节 3 删虚构引文、改写为 B.21 的落地说明；开放问题 1 按 B.21 闭环移出。

## 🟢 建议

### B-11 design 02 砍除清单漏 `.ps1` 脚本对

- 定位：`docs/pebrel-design/02-terminal-core.md` M2.1 砍除清单（仅列 `connection.sh`、`completion.sh`、`proxy.ps1`）；对照 `docs/pebrel-refactor/02-terminal-core.md` 不采纳点 10（仅列 `connection.ps1`、`completion.ps1`）
- 问题：pebrel 一手核实 `nebula_terminal/src/tty/` 两对脚本都在（connection.ps1+sh、completion.ps1+sh，另 proxy.ps1），spec 与设计各缺一对，并集才完整。
- 影响：M2.1 砍除清单不全（禁名门禁可兜底拦截残留 .ps1，但清单本身应全）。
- 修法：M2.1 清单补齐 `connection.ps1` / `completion.ps1`。

### B-12 design 08 照抄拷贝清单多处表格被空行截断

- 定位：`docs/pebrel-design/08-ai-assistants.md` 照抄拷贝清单（AI assistant 节 29/30、32/33、34/35 行间，数学渲染节 39/40、43/44、46/47 行间等）
- 问题：Markdown 表格行之间插入空行，渲染时断成多表/散段。
- 影响：文档可读性，清单纯文本核对仍可行。
- 修法：删除表内空行。

## 已核通过项

- **采纳点落实**：spec 02（61 条）/ 03（43 条）/ 04（33 条）/ 08（53 条）采纳点在对应设计篇全承接（合并表述可接受）；design 04 照抄清单逐条显式编号 33 条；无静默丢失。
- **不采纳点零复活**：多平台 / SSH / Lua / Quick terminal / mux 驻留 / 化学生物渲染 / 9 语言 / 亮主题 / legacy OpenGL / 兼容层 / CI 编排，grep 全部命中均处于「裁剪 / 不迁 / 砍除」语境。
- **越权砍项**：除 B-9（WSLENV 待裁）外未发现砍掉 spec 采纳或用户已决项。
- **产品定位 7 约束**：Windows 原生 / 单窗单实例 / 仅暗色 / GPU 加速 / Ctrl+Shift+C / pwsh→powershell→cmd 回退 / 面向所有 AI CLI，设计与 spec 零违背。
- **行号引用禁令**：`docs/pebrel-refactor/` 与 `docs/pebrel-design/` 两目录对仓内源码/测试的行号引用零命中。
- **pebrel 一手事实核对通过**：AgentKind 27 变体、ProviderKind 13 变体、AGENTS 9 家、FOREIGN_HOOK_RUNNERS 两键、AiTurnOutcome 6 态、USER_ABORT_CODES 四码、INTERACTIVE 表、MAX_PTY_SESSIONS=32、ConPTY flags 矩阵（0x2/0x4/0x8）、TermMode bits（1<<15/16/17/23/24）、DA1 双身份（Term 自答 `?6c` / ConPTY priming `?61c`）、session v4 字段名（ratio_permille / boot_attempts / clean_exit / quarantine / MAX_BOOT_ATTEMPTS）、mux 400+700ms、gpui_shell 40×25ms、NEBULA_GHOST_MAX=96、READ_BUFFER 1MB + MAX_LOCKED_READ 64KB、Colors COUNT=269、OSC 常量族（MAX_PAYLOAD 4096 / 图片 12MB / 16M 像素 / hook 96KB）、win32_input_matrix 三件套、architecture notes 2 条、docs/skills/pebrel-runtime 与 runtime-api-v1.schema.json 存在。
- **spec 失效落点扫描**：除已知两项（`docs/release-notes/`、`docs/skills/`，均已经 D12-3 + design 12 改造节 2 登记）外无新增未登记项。
- **git 子进程误记残留扫描**：除已知 spec 07 采纳点 41（git2 误记，已经 D07-1 重裁）外无新增；SPEC.md 两处「git 子进程封装保留」为前向决策语言，与 D07-1 一致。

## 统计

- 🔴 阻塞：0
- 🟡 应修：10
- 🟢 建议：2
