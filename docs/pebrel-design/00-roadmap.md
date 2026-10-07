# pebrel 重构实施路线图（00-roadmap)

> 本文件是 `docs/pebrel-design/` 的总路线图：阶段序列、点亮顺序、过渡期不可用清单、出口标准、跨领域不变量。各分片设计篇（NN-*.md）细化本领域设计，并在文末回填自己的阶段归属与出口标准——冲突时以本文件为骨架、以分片篇为细节。
>
> 前提：spec(`docs/pebrel-refactor/`,baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e`）已定采纳/不采纳；本路线图不重复裁定，只排「怎么落地」。

## 绿灯语义（全程统一）

每阶段出口 = **可机验项全绿**:`cargo check` / `cargo test` / 架构门禁（file-budgets 棘轮、dependencies.toml 依赖方向、禁名回流）通过。功能完整性**不作要求**——逐 crate 迁入期间产品功能缺失是预期态，用户以历史版本过渡。

禁止模糊出口（「功能可用」「体验正常」类措辞一律不得作为出口标准）。

## fork 执行路径（已定：逐 crate 迁入，禁大爆炸）

1. 每个 crate 独立迁入：拷贝 pebrel 对应 crate → 裁剪（砍不采纳项：unix/SSH/远程/Lua/化学/生物/亮主题/兼容层等，spec 不采纳总表为准）→ 改名（改名映射单点表见 01-arch-baseline 篇）→ 测试转绿 → 落地。
2. 依赖序：`slterm_settings` → `slterm_split` → `slterm_hook` → `slterm_completions` → `slterm_terminal` → `slterm_app`(`slterm_config` ±derive 的落位裁定在 M1 内完成，归 01 篇）。
3. 每步必须可编译可测：裁剪即删，不挂 feature 兼容、不留双名（无历史用户）。
4. 旧栈（`src/` + `src-tauri/` + npm 族）M0 一次性全删，不与新 workspace 并存；slTerminal 自有资产（pty 五件套、plan_balance、安全审计、settings 写通道底座等）从 git 历史与 spec 分片提取，按分片篇的移植设计并入。

## 阶段序列

| 阶段 | 内容 | 可用性状态 | 出口标准（可机验） |
|---|---|---|---|
| **M0 fork 基线** | 删 Tauri 全栈（`src/`、`src-tauri/`、npm 族、wdio/vitest 配置）;建 cargo workspace 骨架（rust-toolchain 钉版、edition 2024、GPUI 依赖钉 Kuddev 双 rev + 补丁注释治理）;LICENSE MIT→GPL-3.0 + THIRD-PARTY-NOTICES/licenses/ 合规族迁入；架构门禁三件套落地（含禁名表 = 禁 nebula/pebrel 回流） | 无可运行产品 | `cargo check` 过（空壳 workspace)；门禁三件套脚本自测过 |
| **M1 底库迁入** | `slterm_settings` → `slterm_split` → `slterm_hook` → `slterm_completions` 逐迁（含裁剪/改名/单测）;`slterm_config` 落位裁定 | 无可运行产品 | `cargo test` 全绿；禁名门禁过；dependencies.toml 方向校验过 |
| **M2 终端核心** | `slterm_terminal` 迁入：vte/OSC tee/Grid/三层渲染合同/boxdraw/ConPTY 侧载/输入/选择；slTerminal pty 五件套并入（DA1/DSR 接管、ConPTY flags 矩阵、Win10 NuGet 捆绑、Job Object、SPAWN_LOCK、shell 白名单、pwsh EncodedCommand) | 无可运行产品 | `cargo test` 全绿（含 ConPTY 集成用例、win32 输入矩阵基线） |
| **M3 app 骨架点亮** | `slterm_app` 最小链路：GPUI 窗口 + 单终端 pane + ConPTY 直连跑通默认 shell（壳即前端，消费 RuntimeHub 状态权威） | **首个可运行态**：能开窗口打字；无分屏/无 AI/无设置页/布局不持久 | 虚拟窗口 UI 测试（终端渲染关键路径）过；`cargo test` 全绿 |
| **M4 AI CLI 集成** | hook 三层拓扑（slterm-hook 小进程 + 命名管道 + 有界仲裁）、安装权威与 9 家一等安装器、AgentKind 27 家、agent_detection 屏幕规则、ai_sessions、per-pane 环境契约；SEC-12 statusline 审查并入 | AI 检测/事件链路点亮 | hook 链路契约测试过（含身份核验/有界重排）；`cargo test` 全绿 |
| **M5 工作区与布局** | 分屏树接入 app、WorkspaceTab 三件套、session v4（下次启动恢复布局）、dock 嫁接 | 分屏/多 tab/布局恢复点亮 | session 往返序列化测试过；分屏树纯函数测试全绿；UI 测试过 |
| **M6 设置/主题/i18n** | RuntimeSettings + JSON 持久化（写通道底座移植）、GUI 设置页、主题语义槽 + WCAG 校验（仅暗色）、i18n 双语静态生成 | 设置页/主题/双语点亮 | 设置读写往返测试、WCAG 对比度校验测试、i18n 键集硬合同测试过 |
| **M7 Runtime API + 单实例** | loopback JSON Lines 控制面、版本化信封、generation 绑定、state_change_seq 等待基线、SKILL.md 自发现、`slterm` CLI 资源动词族；单实例移交（二次启动还原窗口+新开 tab；关窗即退出） | 外部可编程控制点亮 | Runtime API conformance 测试过；单实例移交测试过 |
| **M8 文件与编辑** | file_editor 双模态、file_tree、tab 路由、path_bar、回收站删除；git gutter/通用 diff 新建项 | 编辑器/文件树点亮 | 编辑器内核测试（撤销/outline/虚拟化）全绿；UI 测试过 |
| **M9 AI 辅助** | 补全引擎（历史 ghost/git/scripts 语义源）、AI assistant（默认关闭）、数学渲染管线 | 补全/建议条/数学点亮 | 补全引擎测试、should_suggest 规则表测试、数学 IR/layout 测试过 |
| **M10 系统集成 + 安全** | 托盘/AUMID toast/OSC 9;4 任务栏/自动更新/开机启动/窗口特效；凭据三件套、AES-256-GCM 加密备份；plan_balance 移植（token 不出凭据域） | 系统面与凭据面点亮 | 凭据域边界测试（类型层无 token 字段/日志不插值）、备份往返测试过；`cargo test` 全绿 |
| **M11 打包 + 收尾** | Inno 安装器 + zip 副形态、体积工程（opt-level="s" + 热路径 O3 + 预算钉测试）；根 CLAUDE.md 族重写（新架构现实）、adr/CONTEXT 沉淀、docs/ 临时稿删除 | 发布态 | 安装器构建 + 新鲜度核验链过；体积预算钉测试过；仓内无 docs/ 引用残留 |

阶段内顺序与增删由对应分片篇细化；阶段归属冲突时回 roadmap 协调。

## 过渡期不可用清单（用户以历史版本过渡）

- **M0–M2**：仓库无产品可运行（纯库态）。
- **M3**：仅单终端；分屏、AI 集成、设置持久化、布局恢复、编辑器、补全、托盘、更新全部缺席。
- **M4–M10**：按上表逐领域点亮，未点亮领域功能缺席；不设功能兜底。
- **M11** 前：无安装器，仅 cargo 构建产物。

## 跨领域不变量（任何阶段不得违反）

1. **原子写**：一切持久化写盘（settings JSON / session v4 / runtime.port / 凭据文件 / 备份）= 临时文件 + rename；损坏三态处理。
2. **凭据不出凭据域**：token/key 类型层隔离（无 token 字段出域）、Zeroizing 即用即焚、日志不插值；SEC-18 红线延续（真实凭据禁入 git 追踪文件）。
3. **命名单源**:crate `slterm_*`、环境变量 `SLTERM_*`；改名映射以 01 篇单点表为准；禁名门禁（nebula/pebrel 回流）常挂。
4. **仅 Win10/11**：业务不撒 `#[cfg]`，平台分支收敛于 pty/平台封装模块；ConPTY 侧载优先 + Win10 NuGet 捆绑回退。
5. **关窗即退出**：无 mux 驻留；单实例移交（loopback）是唯一跨进程链路。
6. **定位约束**：仅暗色；GPU 加速渲染；复制 = Ctrl+Shift+C(Ctrl+C 保留中断）；默认 shell pwsh → powershell → cmd 回退；面向所有 AI CLI 调优（27 家一等支持）。
7. **面板/配色/布局/会话元数据单点**：原 Tauri 时代硬约束以 Rust 形态重建（封闭枚举面板类型 + 单一布局转换函数 + TerminalRegistry 对应物），具体形态归 05 篇。
8. **测试覆盖门禁**：改动代码可自动化部分全量自动化测试；不可自动化部分登记豁免（豁免登记制度延续，形态归 11 篇）。

## 引用纪律

- 引 pebrel：符号名 + 文件路径（baseline 钉死），禁行号。
- 引本仓源码：符号名；引测试：例名。
- 本目录文档为临时稿，开发完成后删除——仓内（docs/ 之外）禁止引用本目录任何文件。
