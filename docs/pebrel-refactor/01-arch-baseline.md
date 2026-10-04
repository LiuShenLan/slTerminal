# pebrel 重构优化 spec · 分片 01：架构与基线

baseline commit：`e537d528c508e8607d0f5f9fd25e5902f40d661e`（pebrel，Rust + GPUI 模块化单体）

## 优化面

本分片覆盖七件事：

1. **fork 基线与 commit 记录方式**——baseline commit 落盘位置、后续增量参考操作法。
2. **crate 重组**——pebrel workspace 九名成员逐一裁定（保留改名 / 合并 / 删除），给出目标 workspace 形态。
3. **总删减清单**——所有要砍的目录 / crate / feature / 第三方 patch，每条注明位置与理由。
4. **依赖方向与 ownership map**——slTerminal 版目标依赖方向与模块 owns / must-not-become 表。
5. **GPUI 依赖钉版**——gpui / gpui_platform / gpui-component(-assets) 的 git 依赖钉版与根 Cargo.toml 补丁注释治理。
6. **edition / toolchain**——rust 1.97.1 + edition 2024 对齐方向。
7. **slTerminal 侧现状对照**——src-tauri 各模块与 src 前端在单进程 GPUI 世界的归宿原则。

## 采纳点

### 基线与版本治理

1. **照抄：fork 基线 commit 纪律**。pebrel 根 Cargo.toml 的「GPUI v1.16.1 固定基线」注释块（`[workspace.dependencies]` 的 `gpui` / `gpui_platform`、`[patch.crates-io]` 上方）确立了整套纪律：完整 SHA 钉死、禁 branch / tag / 通配解析、固定点审计信息（上游来源 + 镜像分支名）随注释留存、薄补丁逐条提交并记撤销条件。slTerminal 落盘方式：baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 写入本 spec 头部与根 CLAUDE.md；后续增量参考一律 `git log e537d528..HEAD` 对照；pebrel 代码直接拷贝进当前仓库再修改，沿用本仓库 git log，不加 remote / submodule。
2. **照抄：rust-toolchain.toml 钉版**。pebrel `rust-toolchain.toml`（channel + minimal profile + rustfmt / clippy components）与根 Cargo.toml `[workspace.package]` 的 `rust-version` / `edition` 双写。slTerminal 对齐 rust 1.97.1 + edition 2024（详见「优化方向」）。
3. **参考：发布 / 调试 profile 分层优化**。pebrel 根 Cargo.toml `[profile.release]`（opt-level "s" + 体积预算 <30MB）、`[profile.release.package]`（resvg / rustybuzz / taffy / ttf-parser / smol / gpui / gpui-component / nebula_terminal / image 钉回 O3，热路径手感不受体积编译影响）、`[profile.dev]` + `[profile.dev.package]`（外部依赖统一 O2、塑形布局热点 O3、workspace 成员增量编译）。理由：debug 壳承担真实终端负载时 O0 的 VT 解析与塑形延迟可感知，这套治理经验直接适用于「永远用 debug 构建产物测试」的 slTerminal 习惯。

### 架构形状与门禁

4. **照抄：模块化单体 + 单向依赖方向**。pebrel `docs/architecture.md` Shape 节：**composition/UI → application capabilities → shared domain rules**；平台与 I/O 细节只做适配，领域 crate 不得依赖视图；这是责任模型而非强制重命名。slTerminal 单进程化后此方向天然成立（无 IPC 边界可躲），照抄为架构根约定。
5. **照抄：分层分类 + 机械依赖门禁**。pebrel `architecture/dependencies.toml` 把每个 workspace 成员分类为 core / application / hook / lab，分别声明 normal / build / dev 本地依赖白名单；`renderer_packages` 清单禁止渲染包进入 core 生产依赖；边必须无环（config ↔ config_derive 的 dev 环为显式合法例外）；新成员必须先有分类与 source root。slTerminal 版目标形态见「优化方向」。
6. **照抄：ownership map 表**。pebrel `docs/architecture.md`「Current ownership map」表的 owns / must not become 双列形态——每域一句话说清「拥有什么」与「禁止退化为什么」。比单纯列模块清单更能防职责漂移，slTerminal 版在「优化方向」给出初版。
7. **照抄：文件规模红线机制**。pebrel `architecture/file-budgets.txt`（limit + root + 存量豁免额度的单一来源）+ `docs/project-constraints.md` §1：2000 物理行硬限、800 行仅提示、存量超标文件按实测值登记只减不增、删文件即销额度、禁通配额度、禁 part1/part2 拆分过检。slTerminal 按同机制重建（初版无存量豁免）。
8. **参考：能力模块生命周期约定**。pebrel `docs/architecture.md`「Feature modules and lifecycle」：capability 按 entry/facade + 具名子模块（model / adapter / view / tests，按真实职责命名）组织；跨能力调用走显式命令/结果而非互改内部集合；第二个真实实现出现前不造 trait；背景工作必须带 owning scope、取消/过期策略。slTerminal 直接采纳为模块约定。
9. **参考：扩展检查单**。pebrel `docs/architecture.md`「Extension checklist」——新设置 / 新语言 / 新面板 / 新平台 / 新依赖各走固定登记路径（其中「新平台支持」一条在 slTerminal 恒为 Windows，但「不支持的行为必须显式而非静默成功」的原则保留）。
10. **参考：交互反馈工程约束**。pebrel `docs/project-constraints.md` §5（尤其 §5.2 复制操作合同：复制必须有可感知反馈、成功失败不能只用颜色区分、复制默认不带装饰、单一权威实现）——与 slTerminal「复制 = Ctrl+Shift+C」定位直接吻合，作为 GPUI 壳 UI 评审合同采纳。

### crate 重组裁定（逐成员）

11. **保留改名：`nebula_terminal` → `slterm_terminal`**（照抄）。pebrel `nebula_terminal`（grid、VT 处理、终端/PTY 行为，`docs/architecture.md` ownership 表；存量豁免见 `architecture/file-budgets.txt` 的 `nebula_terminal/src/term/mod.rs`）是纯领域 core，无渲染依赖，改名保留。
12. **保留改名：`nebula_split` → `slterm_split`**（照抄）。split tree / 几何 / 导航规则，core + 零生产依赖契约（`architecture/dependencies.toml` 的 `zero_production_dependencies`），改名保留。
13. **保留改名：`nebula_settings` → `slterm_settings`**（照抄形态，改持久化）。运行时设置 + 语言注册表 + 共享偏好契约；持久化介质由 nebula_settings.txt / Lua 改为 JSON（已定全局决策），零生产依赖契约保留。
14. **保留改名：`nebula-completions`（目录）/` pebrel-completions`（crate）→ `slterm_completions`**（照抄）。补全匹配与呈现无关的结果计算，core 层、不碰终端视图——「面向所有 AI CLI 调优」的补全领域正好落在这里。
15. **保留改名：`nebula_hook` → `slterm_hook`**（照抄）。小进程 / 生命周期 hook 桥，hook 层 + 零生产依赖，改名保留。
16. **保留改名：`nebula_app` → `slterm_app`**（照抄）。GPUI 壳、编排、平台适配、i18n、build 生成全收于此；`nebula_app/src/gpui_shell`、`product_ui` facade 概念、`main.rs` feature 选择模块的模式一并迁入。
17. **合并重估：`nebula_config` + `nebula_config_derive`**（参考保留）。Lua 砍后 config 抽象只剩 serde 校验/默认值职责，裁定：保留为 `slterm_config` + `slterm_config_derive` 薄层，若落地时发现无第二个实现需求则合并进 `slterm_settings`，此二选一在 crate 落位时定。
18. **删除：`nebula_gpui`**（照砍）。组件验收 lab，`architecture/dependencies.toml` 中 layer = "lab"、不依赖产品——已定不采纳实验场。
19. **删除：`mobile/link`**（照砍）。mobile 通信链路 crate，随 mobile/ 整体删除；`architecture/dependencies.toml` 中它被列为 core 仅是历史遗留分类，无独立保留价值。

### GPUI 依赖钉版

20. **照抄：gpui / gpui_platform 钉版**。pebrel 根 Cargo.toml `[workspace.dependencies]`：`gpui = { git = "https://github.com/Kuddev/zed", rev = "fc05d637cc7029d75de051fd7f52c1a0fb8fa6b4", version = "=0.2.2" }`、`gpui_platform` 同 URL 同 rev + `version = "=0.1.0"` + font-kit feature。同 URL + 同 rev 的铁律（Cargo source identity 含 URL，混 URL 即使同 SHA 也解析成两套互不兼容类型）一并照抄。
21. **照抄：gpui-component / gpui-component-assets 钉版**。同文件：`git = "https://github.com/Kuddev/gpui-component", rev = "fc5f5cf63dd80686dafacd2a6e37345bbd1dc7ba"`，版本 `=0.5.2` / `=0.5.1`。
22. **照抄：根 Cargo.toml 补丁注释治理**。pebrel 根 Cargo.toml 约 130 行注释块整体移植思路：固定基线节（版本配对、上游/镜像审计点、禁移动基线 branch/tag）、窗口生命周期/字体内存等薄补丁逐条记录（原因、范围、上游撤销条件）、产品补丁清单逐条迁移验收（禁一次性混入）、上游行为备忘（Scrollable 的 w_full 约定等调用侧须知）。slTerminal 侧钉版参数照抄上述 rev，注释结构按本仓现状重写、随补丁增删同步维护。
23. **照抄：数学公式渲染栈**。`nebula_app/src/math/`（parse / validate / layout / compile / cache）与 `gpui_shell/math_view` 整体迁入，gpui-component 的 TextView 数学钩子补丁（根 Cargo.toml 补丁清单第 2 条）保留接线。细节归分片 08，此处仅登记归属。
24. **照抄：自动更新两阶段 handoff**。`nebula_app/src/update_check/` 与 `update_download/` 的两阶段移交（检查与下载分属不同生命周期，避免下载逻辑常驻）照抄保留。细节归分片 09。
25. **照抄：加密备份**。`nebula_app/src/encrypted_backup/` 属安全三件套之一，照抄保留。细节归分片 10。

### 依赖方向与 ownership（slTerminal 目标版）

26. **照抄方向、重写内容：目标依赖方向**（依据 `architecture/dependencies.toml` 分类法）：
    - core 层：`slterm_terminal`、`slterm_split`、`slterm_settings`、`slterm_completions`、`slterm_hook`（+ 待定 `slterm_config`）——不依赖 app、不依赖渲染包；settings / split / hook 保持零生产依赖契约。
    - application 层：`slterm_app` 单向依赖全部 core。
    - hook 层：`slterm_hook` 独立小进程桥。
    - 无 lab 层。
    - `renderer_packages = ["gpui", "gpui_platform", "gpui-component", "gpui-component-assets"]`（winit / glutin / crossfont 是否入清单视 GPUI 依赖图而定，原则不变：渲染包禁入 core）。
27. **参考重写：ownership map（slTerminal 版初稿，随落地演化）**：
    | 域 | Owns | Must not become |
    | --- | --- | --- |
    | `slterm_settings` | 运行时设置、JSON 持久化契约、语言注册表 | UI/widget 库 |
    | `slterm_split` | 分屏树、几何、导航规则 | 窗口管理或渲染 |
    | `slterm_terminal` | 网格、VT 处理、PTY 行为 | 产品面板或 GPUI 状态 |
    | `slterm_completions` | 与呈现无关的补全匹配与候选计算 | 终端视图所有权 |
    | `slterm_hook` | CLI 生命周期 hook 桥 | 应用依赖容器 |
    | `slterm_app/src/gpui_shell` | GPUI 视图、UI 状态、命令、订阅 | 第二份设置/领域实现 |
    | `slterm_app/src/platform` | Windows 能力声明与原生适配器 | 无关逻辑的倾倒场 |
    | `slterm_app/src/i18n` | 静态查表、locale 解析（en + zh-CN） | 运行时目录解析或 UI 所有权 |

## 不采纳点

### 多平台与远程（已定全局决策，逐条落位）

1. **`mobile/` 整目录**——android、link、protocol、relay、ssh、tools 全砍；workspace 成员 `mobile/link` 同步删除。位置：pebrel `mobile/`。理由：mobile 为已定不采纳项。
2. **SSH 全家**——`nebula_app/src/ssh.rs`、`ssh_session/`、`ssh_sftp.rs`、`ssh_credentials.rs`、`ssh_profiles.rs`、`ssh_prompt.rs`、`ssh_proxy.rs`、`remote_dirs.rs`、`backup_remote.rs`、`mobile/ssh`、`ai_hook` 内的 SSH transport、`platform/ssh_agent.rs`、ssh_session 的 agent / integration 模块。理由：SSH / 一切远程为已定不采纳项；ownership 表中对应行（`ssh_session/agent.rs`、`ssh_session/integration.rs` 等）随删。
3. **macOS / Linux 代码**——`nebula_app/src/macos/`、platform 的 unix 分支、i18n 目录中非 en / zh-CN 语言。理由：仅 Win10/11 定位。
4. **`[patch.crates-io]` 的 `x11-clipboard`**（pebrel 根 Cargo.toml）——X11 剪贴板补丁，Linux-only。砍。
5. **`third_party/winit-0.30.13` 路径补丁**（pebrel 根 Cargo.toml `[patch.crates-io]` 的 `winit = { path = ... }`）——该补丁为 legacy OpenGL 旧壳 backport Win11 mixed-DPI 修复；legacy 壳为已定不采纳项，GPUI 自带窗口层不消费此补丁。连 `third_party/` 目录一并砍。
6. **`packaging/linux`、`packaging/macos`**——砍。`packaging/scoop`——砍（打包已定 Inno）。`packaging/windows/AppxManifest.xml`（MSIX）——砍。pebrel `packaging/windows/` 仅剩 Appx 清单可参考价值为零；slTerminal 保留现有 Inno 打包体系（`.claude/package.ps1`），pebrel 侧无可迁移的 Inno 资产。

### 功能面（已定全局决策）

7. **Lua 配置栈**——`mlua` 依赖（pebrel 根 Cargo.toml `[workspace.dependencies]`）、`nebula_config` 的 Lua 后端、`nebula_app/src/plugins/`、`runtime_api` 的 Lua 暴露面、nebula_settings.txt 读写路径。理由：Lua 配置为已定不采纳项；设置持久化改 JSON，仅存 serde + schema 校验层。
8. **mux 驻留保活**——`nebula_app/src/mux.rs`、`daemon.rs`、`session.rs` 的驻留语义。理由：关窗即退出、仅留单实例移交为已定决策；mux 进程角色（`docs/architecture.md` Shape 节提到的独立 mux 进程）不成立。注意区分：单实例移交（第二个实例把会话交给首个实例）与 mux 驻留是两件事，前者保留、后者砍。
9. **Quick terminal**——砍（已定）。
10. **化学 / 生物渲染**——`nebula_app/src/chemistry.rs`、`scientific_corpus.rs`。理由：已定不采纳。
11. **legacy OpenGL 旧壳**——`nebula_app/src/display/`、`renderer/` 的 legacy 渲染路径、`product_renderer.rs`、`product_input.rs`、`product_ui` 中的 legacy facade 分支、`window_context.rs` 旧壳部分、窗口动画 `display/animations.rs`。理由：legacy 壳为已定不采纳项；仅迁入其中已被 product facade 复用的提取模型。
12. **`nebula_gpui` 实验场**——砍，见采纳点 18。

## 优化方向

1. **基线落地**：本 spec 头部与根 CLAUDE.md 写入 baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e`；pebrel 代码拷贝入当前仓库后，一切增量以 `git log e537d528..HEAD` 对照评审；不加 remote / submodule，git 历史沿用本仓。
2. **workspace 目标形态**：单一 `[workspace]`，成员 `slterm_app`（application）、`slterm_terminal` / `slterm_split` / `slterm_settings` / `slterm_completions` / `slterm_hook`（core，settings/split/hook 零生产依赖）、`slterm_config` + `slterm_config_derive`（保留或并入 settings，crate 落位时定）；`resolver = "2"`，`[workspace.package]` 统一 `edition = "2024"`、`rust-version = "1.97.1"`。toolchain 以仓内 `rust-toolchain.toml` 钉 channel = "1.97.1"（minimal profile + rustfmt + clippy），取代当前依赖全局 stable 1.96.0 的状态——GPUI 钉版对编译器版本敏感，仓内钉版是唯一可复现路径。
3. **迁移顺序原则**：先钉版（toolchain + GPUI 依赖 + 注释治理），再落 workspace 骨架（空 crate + dependencies.toml 分类 + file-budgets 机制），后按域迁入 pebrel 代码（砍清单内的目录不拷），每域迁入时同步建 ownership map 条目与依赖白名单条目。
4. **依赖治理前置**：`architecture/dependencies.toml` 式清单与检查脚本在首个 crate 迁入前建立；渲染包禁入 core、core 不依赖 app 两条作为机械门禁从第一天生效，避免迁移完成后返工。
5. **slTerminal 现状归宿原则**（src-tauri 模块 → 单进程世界）：
   - **融入 core**：PTY / 终端行为概念入 `slterm_terminal`；`pty/` 的 Windows 特有进程管理（job object、管道）作为其 adapter；`fs` / `git` / `notify` / `projects` / `agent_dirs` / `agent_history` / `background_tasks` 的领域规则以模块形态入 `slterm_app` 对应能力域（platform adapter 与领域模型分离）。
   - **融入 app 能力域**：`plan_balance` / `hooks` / `settings` / `preview` 直接对应 app 内同名能力；`settings.rs` 的 JSON 持久化与 `slterm_settings` 的契约合并。
   - **消亡**：Tauri 命令层（`lib.rs` 的 `generate_handler!`、`state.rs` 的 IPC 状态容器）、`src/ipc/` 全部 invoke 封装、`src/types/` 的 ts-rs DTO 生成（CP-024 单源机制随两进程模型终结）、`tauri-plugin-*` 依赖（剪贴板/通知/对话框改走 GPUI 原生实现）。
   - **整体替代**：`src/` 前端（React / xterm.js / dockview / vite / zustand）由 GPUI 壳整体替代，不复存在；前端专属的「面板封闭」「配色单点」「布局单点」等约束的**问题域**（面板注册、主题 token、布局序列化）在 GPUI 壳内以 Rust 形态重建，约束文本不迁移；四级测试金字塔中 L2/L3 随前端消亡，L1（cargo）扩展为绝对主力，L4 以 GPUI 虚拟窗口测试支持（pebrel 已验证的 test-support 补丁路径）重建等价能力。
   - **产品定位不变**：Windows 原生、单窗口单实例、仅暗色、GPU 加速、复制 = Ctrl+Shift+C、默认 pwsh→powershell→cmd 回退、面向所有 AI CLI 调优（27 家 CLI 一等支持）——上述迁移均以这七条为筛选与验收基准。
6. **注释治理惯例**：根 Cargo.toml 的补丁注释块、「负载裕度」式改动点注释、依赖 pin 的理由注释，均按 pebrel 密度惯例执行——每处 pin / patch / profile 特判必须自带原因、影响面与撤销条件，不留裸 magic。
