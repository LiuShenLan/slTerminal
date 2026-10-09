# 01 架构基线详细设计

> pebrel-design 分片 01/12。上游 spec:`docs/pebrel-refactor/SPEC.md`（总表）+ `docs/pebrel-refactor/01-arch-baseline.md`（spec 分片）；骨架:`docs/pebrel-design/00-roadmap.md`。
>
> 引上游一律符号名 + 文件路径（baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死），禁行号。本分片承载两个全局单点职责：**改名映射单点表**（本篇「改造 / 移植 / 新建设计」节，后续 11 篇只许引用、不得另立）与**类型锚点规则**（本篇「边界与不变更项」节）。

## 目标形态

终态 = 单一 `[workspace]` 的 cargo workspace（仓库根 `Cargo.toml`）+ 架构门禁三件套 + 因果 note 治理，全部机制本地可跑（无 CI)。

### workspace 终态组成

| 目录 = package 名 | 产出 | 层 | 生产依赖（workspace 本地边） | 职责 |
| --- | --- | --- | --- | --- |
| `slterm_app` | lib+bin `slterm` | application | slterm_terminal / slterm_settings / slterm_split / slterm_completions | GPUI 壳、编排、平台适配、i18n、build 生成 |
| `slterm_terminal` | lib | core | 无本地边 | 网格、VT、PTY 行为（细节归 02 篇） |
| `slterm_split` | lib | core | 无（零生产依赖契约） | 分屏树、几何、导航（归 05 篇） |
| `slterm_settings` | lib | core | 无（零生产依赖契约） | 运行时设置 + JSON 持久化契约（归 06 篇）;serde 校验/诊断薄层并入本篇（B.1) |
| `slterm_completions` | lib | core | 无本地边 | 补全匹配（归 08 篇） |
| `slterm_hook` | bin `slterm-hook` | hook | 无（零生产依赖契约） | AI-CLI 生命周期 hook 小进程（归 03 篇） |

依赖方向（照抄 pebrel `docs/architecture.md` Shape 节：**composition/UI → application capabilities → shared domain rules**):

- core 不依赖 app、不依赖渲染包；`slterm_settings` / `slterm_split` / `slterm_hook` 零生产依赖契约机械强制。
- application 单向依赖全部 core；无 lab 层。
- `slterm_hook` 是独立小进程桥，slterm_app 不依赖 slterm_hook crate（管道服务器与协议解析住在 app 的 ai_hook 域，归 03 篇）。
- 边的无环由 dependencies.toml 检查器强制。

### 根 Cargo.toml 关键块（草稿）

```toml
[workspace]
members = [
    "slterm_app",
    "slterm_terminal",
    "slterm_completions",
    "slterm_hook",
    "slterm_settings",
    "slterm_split",
]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.97.1"

[profile.release]
lto = "thin"
debug = 0
strip = "symbols"
incremental = false
codegen-units = 1
opt-level = "s"            # 体积预算与热路径 O3 钉包清单归 12 篇首版实测校准

[profile.dev]
codegen-units = 16
debug = "limited"
split-debuginfo = "unpacked"

[profile.dev.package]
"*" = { opt-level = 2 }    # 因果链照抄 pebrel：debug 壳承担真实终端负载，
                           # O0 的 VT 解析/塑形延迟可感知；成员自身保持增量 debug

[workspace.dependencies]
gpui = { git = "https://github.com/Kuddev/zed", rev = "fc05d637cc7029d75de051fd7f52c1a0fb8fa6b4", version = "=0.2.2" }
gpui_platform = { git = "https://github.com/Kuddev/zed", rev = "fc05d637cc7029d75de051fd7f52c1a0fb8fa6b4", version = "=0.1.0", features = ["font-kit"] }
gpui-component = { git = "https://github.com/Kuddev/gpui-component", rev = "fc5f5cf63dd80686dafacd2a6e37345bbd1dc7ba", version = "=0.5.2" }
gpui-component-assets = { git = "https://github.com/Kuddev/gpui-component", rev = "fc5f5cf63dd80686dafacd2a6e37345bbd1dc7ba", version = "=0.5.1" }
```

`[patch.crates-io]` 整段不建：x11-clipboard patch 与 `third_party/winit-0.30.13` 路径补丁随 Linux-only / legacy 壳一并砍（spec 分片 01 不采纳点 4/5)。

GPUI 钉版四要点（照抄 pebrel 根 Cargo.toml「GPUI v1.16.1 固定基线」注释块的纪律，非数值）:

1. 完整 SHA 钉死，禁 branch / tag / 通配解析；branch/tag 名只作审计信息随注释留存。
2. `gpui` 与 `gpui_platform` 同 URL + 同 rev——Cargo 的 source identity 含 URL，混 URL 即使同 SHA 也解析成两套互不兼容类型。
3. 所有 Zed 系依赖统一 Kuddev URL（gpui-component 内部五个 Zed 引用同步），防双 gpui 类型。
4. 薄补丁逐条记录：原因、范围、上游撤销条件；新补丁从固定基线开独立分支、逐条提交、更新 exact rev，禁一次性混入、禁移动基线 branch/tag。

slTerminal 落盘时注释结构按本仓现状重写，固定点审计信息照抄：上游 `zed-industries/zed` v1.16.1、镜像分支 `nebula-v1.16.1-owned-base`（纯上游镜像）、组件分支 `nebula-v1.16.1-math` / `nebula-v1.16.1-test-support`（数学薄桥 + 虚拟窗口测试支持）。**注意：注释中引用的上游分支名含 `nebula` 字样，属「消亡/来源」语境的审计锚点，豁免禁名门禁（豁免登记见本篇门禁设计）**——这是 fork 注释块与禁名门禁的第一个真实缝合点。

### rust-toolchain.toml

```toml
[toolchain]
channel = "1.97.1"
profile = "minimal"
components = ["rustfmt", "clippy"]
```

因果链照抄 pebrel `rust-toolchain.toml`:GPUI 钉版对编译器版本敏感，仓内钉版是唯一可复现路径；`[workspace.package]` 的 `rust-version` 与 toolchain channel 双写，`edition = "2024"`。

## 边界与不变更项

1. **产品定位七条**（任何阶段不得违反，详见 00-roadmap 跨领域不变量）：仅 Win10/11 原生；单窗口单实例；仅暗色；GPU 加速；复制 = Ctrl+Shift+C；默认 shell pwsh → powershell → cmd；面向所有 AI CLI 调优（27 家一等）。
2. **单进程 GPUI 模块化单体**：无 WebView、无 IPC、无 Tauri 命令、无 Dockview、无 xterm.js——本篇涉及这些词仅在「消亡 / 映射 / 来源」语境。
3. **基线钉死与增量操作法**:baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e`；不加 remote / submodule;pebrel 代码拷贝入本仓后一切增量以 `git log e537d528..HEAD` 逐个评审回采。
4. **类型锚点规则（全局单点，后续 11 篇禁自定义）**——跨领域类型的唯一定义归属表：

   | 类型族 | 锚定篇 | 他篇纪律 |
   | --- | --- | --- |
   | pane/tab 标识、分屏树类型（`PaneId`、`TabId`、`SplitTree` 等） | 05 workspace-layout | 只许 `use` / 经 facade 传参，禁重定义 |
   | `AgentKind`、hook 事件类型（`AiHookEvent` 等） | 03 ai-cli-integration | 同上；27 家枚举不裁剪 |
   | `RuntimeSettings`、主题类型（`TermTheme` / `ReviewedPalette`)、设置键域枚举 | 06 settings-theme-i18n | 键域枚举单源 |
   | session v4 schema 类型 | 05 workspace-layout | 与布局恢复同篇演化 |
   | Runtime API 信封 / 方法类型 | 04 runtime-api | 版本化信封 `deny_unknown_fields` |
   | 凭据类型（provider 凭据、引用字段、尾四掩码） | 10 security | token 不出凭据域的类型层边界 |
   | 终端 `Grid` / revision / OSC 类型 | 02 terminal-core | 三层渲染合同的快照类型 |
   | 补全类型（候选、方言快照） | 08 ai-assistants | 呈现无关 |
   | 测试基础设施类型（虚拟窗口支持、键位矩阵夹具） | 11 testing | — |
   | 通用错误类型（`AppError` / 结果别名）、`brand` 常量表 | **01 本篇** | `slterm_app::error` + `slterm_app::brand`，全 crate 共享 |

   规则：他篇引用归属篇定义；禁重复定义、禁别名漂移、禁字段语义改写；新增跨领域类型时先在本表登记归属。本篇自身的锚点：`brand::{NAME, WINDOWS_APP_ID, DESCRIPTION}` 与 `AppError` 在 `slterm_app` 落位时（M3）建立，但**归属权登记在本篇**，后续篇只消费。

5. **迁移顺序原则**（spec 分片 01 优化方向 3/4）：先钉版（toolchain + GPUI 依赖 + 注释治理），再落 workspace 骨架（空 crate + dependencies.toml 分类 + file-budgets 机制），后按域迁入；门禁三件套在首个 crate 迁入前建立并常挂。
6. **绿灯语义**（00-roadmap):M0/M1 出口只认可机验项，功能完整性不作要求。

## 关键类型与签名

### 门禁三件套脚本接口（照抄 pebrel `scripts/`，形态不变、词表/根目录改造）

**file-budgets 棘轮**——`architecture/file-budgets.txt` 为 limit / root / 存量豁免额度的单一来源；检查器 `scripts/architecture/budgets.py`:

```python
BUDGET_PATH = "architecture/file-budgets.txt"
SOURCE_SUFFIXES = {".rs", ".py", ".ps1", ".mjs", ".js", ".sh", ".lua"}

@dataclass
class Budget:
    limit: int
    roots: tuple[str, ...]
    exceptions: dict[str, int]

    @classmethod
    def parse(cls, text: str) -> "Budget": ...   # 校验:limit 唯一正整数、root 不重叠、
                                                 # 豁免须 > limit 且落在 root 内、禁通配

def check(root, budget, previous=None, read_previous=None):
    # 返回 (errors, notices, counts)
    # 棘轮语义:previous 存在时 limit 不可升、root 不可减、豁免不可新增/抬值;
    # 文件缩到 <= limit 时报错要求销豁免;文件删除即销额度;软目标 800 只提示
```

**依赖方向**——`architecture/dependencies.toml` + `scripts/architecture/dependencies.py`:

```python
POLICY_PATH = "architecture/dependencies.toml"
KINDS = ("dependencies", "build-dependencies", "dev-dependencies")

def load(path: Path) -> dict: ...                # tomllib,禁未知顶层字段
def check(root, policy, source_roots=None, scanned_paths=None):
    # 返回 (errors, members);强制:members 显式列出且与 policy.crates 键集全等、
    # layer ∈ {core, application, lab, hook}、zero_production_dependencies、
    # renderer_packages 禁入 core/hook 生产依赖、core→非 core 生产边非法、
    # 生产边无环
```

聚合入口 `scripts/check_architecture.py`:`run(root, base=None, report=False) -> int`，先 budgets 后 dependencies,`--base <commit>` 启用历史棘轮（对 PR base 拒绝新增/抬额债务）。

slTerminal 版 `architecture/dependencies.toml` 草稿：

```toml
version = 1
renderer_packages = ["gpui", "gpui_platform", "gpui-component", "gpui-component-assets", "winit", "glutin", "crossfont"]

[crates.slterm_app]
layer = "application"
dependencies = ["slterm_terminal", "slterm_settings", "slterm_split", "slterm_completions"]
build-dependencies = ["slterm_settings"]
dev-dependencies = []

[crates.slterm_terminal]
layer = "core"

[crates.slterm_completions]
layer = "core"

[crates.slterm_settings]
layer = "core"
zero_production_dependencies = true

[crates.slterm_split]
layer = "core"
zero_production_dependencies = true

[crates.slterm_hook]
layer = "hook"
zero_production_dependencies = true
```

`renderer_packages` 含 winit / glutin / crossfont 照抄 pebrel 清单——三者随 legacy 壳代码删除后不再出现在依赖图，但清单保留无害且防未来回流（原则：渲染包禁入 core，宁可多列）。

slTerminal 版 `architecture/file-budgets.txt` 草稿（初版零豁免）:

```text
limit 2000
root slterm_app
root slterm_terminal
root slterm_completions
root slterm_hook
root slterm_settings
root slterm_split
root scripts
root tools
```

**禁名回流表**（新建，机制照抄 pebrel `scripts/check_prohibited_names.py` 的四模式 CLI):`scripts/check_brand_regress.py`:

```
usage: check_brand_regress.py staged|message|push [message-file] | range --base BASE --head HEAD
```

- 词表：`nebula` / `pebrel`，词边界 + 大小写不敏感；扫描对象 = 新增行（staged / push / range 逐提交）与提交信息（message / push / range)。
- 逐提交扫描（`scan_pending_commits`）形态照抄：防「先加入、后删除」在 range diff 中被抵消；merge commit 走 combined diff。
- 豁免：`EXEMPT_PATHS` 登记门禁自身、其自测、其夹具、`docs/pebrel-refactor/`、`docs/pebrel-design/`（临时稿目录，引用纪律允许其存在至 M11 删除）、根 Cargo.toml 的 GPUI 固定基线注释块（上游审计锚点含 `nebula-` 分支名）。**豁免按路径精确登记，禁目录级豁免**（照抄 pebrel「不能按目录或整个源码文件豁免」原则）。
- pebrel 侧的三类放行模式（版权归属行、依赖坐标 `git = "..."`、协议兼容名如 `KITTY_*` / `ThemeFormat::Kitty`）中，前两类机制保留（THIRD-PARTY-NOTICES 与 Cargo 依赖坐标必然出现外部名），协议兼容类整体不需要——但 `ThemeFormat::Pebrel`（pebrel 主题导入格式）若 06 篇裁定保留 pebrel 主题导入能力，须在 06 篇按 pebrel 的 `PROTOCOL_COMPATIBILITY_PATTERNS` 形态登记精确豁免模式。

### 品牌与路径常量（新建，`slterm_app::brand` / `slterm_settings::paths`)

```rust
// slterm_app/src/brand.rs
pub const NAME: &str = "slTerminal";
pub const WINDOWS_APP_ID: &str = "com.slterminal.terminal";  // AUMID,消费归 09 篇
pub const DESCRIPTION: &str = "slTerminal — GPU-accelerated terminal for AI CLIs";
```

```rust
// slterm_settings/src/paths.rs(缝合点:改写自 pebrel nebula_settings/src/paths.rs,
// 砍掉 migration.rs——legacy Nebula→Pebrel 迁移链不迁,无历史用户)
fn first_override(...)  // 仅 ["SLTERM_CONFIG_DIR"],双名别名层不迁
pub fn settings_dir() -> PathBuf   // SLTERM_CONFIG_DIR 优先;默认 %APPDATA%\slterm(D2 裁决值)
pub fn settings_path() -> PathBuf  // settings_dir().join("settings.json")——JSON 化已定,txt 文件名不迁
```

`slterm_app::platform::dirs::{data_dir, home_dir}` 照抄 pebrel `nebula_app/src/platform/dirs.rs` 的 `OnceLock` 缓存 + USERPROFILE 形态（含「空串视为未设置」过滤器）,Windows 落点测试由 `windows_path_uses_pebrel_layout` 改写为 `windows_path_uses_slterm_layout`。

### 设置诊断薄层签名（B.1 已裁：并入 `slterm_settings` 单 crate)

照 pebrel `nebula_config/src/lib.rs` 的公开面原样收缩（Lua 后端已砍，只剩 serde 校验/诊断），以 `slterm_settings` 内部模块形态落位，settings 仍满足零生产依赖契约；独立 proc-macro crate 不迁，`ConfigDeserialize` / `SerdeReplace` 按需手实现：

```rust
pub enum UnknownFieldPolicy { ... }
pub enum DiagnosticKind { ... }
pub struct ConfigDiagnostic { ... }
pub fn capture_diagnostics<T>(f: impl FnOnce() -> T) -> (T, Vec<ConfigDiagnostic>);
pub fn report_unknown_field(target: &'static str, field: &str);
pub fn report_deprecated_field(target: &'static str, field: &str, message: &str);
pub fn report_invalid_value(target: &'static str, field: &str, detail: &str);
pub trait SerdeReplace { ... }
```

## 数据流与状态机

门禁三件套的数据流（无状态机）:

1. `check_architecture.py run()` ← 读 `architecture/file-budgets.txt` 得 `Budget`；若有 `--base`,git 读 base 修订的同名文件得 `previous`（历史棘轮参照）。
2. `budgets.check()` ← 遍历 root 下 `SOURCE_SUFFIXES` 文件计数物理行（CRLF/末行无换行一致处理；Cargo 包根的 `target/` 跳过；禁符号链接）→ 对 limit / 豁免 / 棘轮逐条产生 errors / notices / counts。
3. `dependencies.check()` ← 读根 Cargo.toml members + 各成员 Cargo.toml 的三种依赖节（含 workspace 继承解析、alias→package 解析、path 依赖本地归类）→ 建图 → 白名单边校验 + 层方向校验 + `cycles()` DFS 无环检测。
4. `check_brand_regress.py` ← git diff 取新增行 / `git log` 取提交信息 → 词边界正则扫描 → 扣减 `EXEMPT_PATHS` 与精确放行模式 → 命中即非零退出。

四处均为「读声明文件 + 读 git/文件系统 → 纯函数判定 → exit code」，无持久状态；唯一状态载体是 `file-budgets.txt` 与 `dependencies.toml` 两个被 git 追踪的声明文件本身。

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单（pebrel → slTerminal，逐条注明来源与因果链）

| 来源（pebrel 路径） | 落位 | 因果链（为什么照抄） |
| --- | --- | --- |
| `rust-toolchain.toml` | `rust-toolchain.toml` | 仓内钉版 = GPUI 钉版可复现的唯一路径 |
| 根 `Cargo.toml` 的 GPUI 固定基线注释块（纪律结构，非数值） | 根 `Cargo.toml` | 薄补丁逐条可撤销治理；防双 gpui 类型的 URL identity 纪律 |
| 根 `Cargo.toml` `[profile.dev]` / `[profile.dev.package]` 结构 | 根 `Cargo.toml` | debug 壳承担真实终端负载，O0 延迟可感知；与「永远用 debug 构建产物测试」习惯直接咬合 |
| 根 `Cargo.toml` `[profile.release]` 结构（数值归 12 篇校准） | 根 `Cargo.toml` | opt-level s + 热路径 O3 钉包的治理形态 |
| `scripts/architecture/budgets.py` | 同名 | file-budgets 棘轮的完整判定语义 |
| `scripts/architecture/dependencies.py` | 同名 | 依赖方向机械门禁的完整判定语义 |
| `scripts/check_architecture.py` | 同名 | 聚合入口与 `--base` 棘轮接线 |
| `scripts/check_prohibited_names.py` 的四模式 CLI 与逐提交扫描结构 | `scripts/check_brand_regress.py` | 增量防回流机制本身与词表无关，可整体复用骨架 |
| `architecture/notes/AGENTS.md`（因果 note 规则） | `architecture/notes/AGENTS.md` | Status/Context/Evidence/Decision/Rejected/Consequences/Validation/Supersedes/Revisit when 九段结构 + Supersedes 链 + Revisit when 撤销条件，与 adr.md 双轨互链（spec 总表「文档纪律」行） |
| `LICENSE`(GPL-3.0 全文） | `LICENSE` | 派生合规：上游 GPL-3.0,fork 派生必须同许可证传播（SPEC 基线决策 LICENSE 行） |
| `THIRD-PARTY-NOTICES`（结构） | `THIRD-PARTY-NOTICES` | 合规族形态；内容按 slTerminal 实际依赖裁剪归 12 篇 |
| `licenses/`(LICENSE-LATIN-MODERN-MATH 等） | `licenses/` | 内嵌数学字体等的组件许可证必须随包分发；数学字体消费归 08 篇 |
| `docs/project-constraints.md` §1 / §5.2 的机制条款 | 机制落 `architecture/` + 约定入 CLAUDE.md 族 | §1 = 红线机制语义（物理行含注释测试空行、禁 part1/part2、豁免只减不增）;§5.2 复制合同归 GPUI 壳 UI 评审 |
| `docs/architecture.md` 的 Shape / ownership map 双列 / feature lifecycle / extension checklist | `docs/architecture.md` 重写版 | 责任模型防漂移；slTerminal 版 ownership 初稿见 spec 分片 01 采纳点 27,落地随 M1-M3 演化 |

### 改名映射引用

全文改名以本篇「改造 / 移植 / 新建设计」节的**改名映射单点表**为唯一权威；00-roadmap 已指定各篇引用它，本篇之外禁另立映射。

### 缝合点

1. **settings 路径双端引用**:`slterm_app::platform::dirs::data_dir()` 委托 `slterm_settings::settings_dir()`（照 pebrel `nebula_app/src/platform/dirs.rs` 的 `resolve_data_dir` 委托形态）,app 侧只缓存不推导——路径优先级唯一权威在 settings crate。
2. **brand 常量消费**:`brand::WINDOWS_APP_ID` 归 09 篇 toast/AUMID 消费；`brand::NAME` 归 04 篇 CLI(`bin_name = "slterm"`）与 12 篇安装器消费；本篇只锚定值。
3. **GPUI 注释块 × 禁名门禁**：注释块含 `nebula-` 分支名（上游审计锚点），登记为禁名门禁的精确 `EXEMPT_PATHS` 项——「来源」语境豁免的范例。
4. **file-budgets 的 Rust 适配器**:pebrel `nebula_app/tests/file_line_budget.rs` 让 Rust 测试读同一份预算；slTerminal 对应物归 11 篇测试基础设施。
5. **hook 协议头**：线协议 `nebula-hook/1` → `slterm-hook/1`（映射表 P-1),app 侧协议解析与 hook 小进程两侧同步改， eg 03 篇。

## 改造 / 移植 / 新建设计

### 改名映射单点表（全局唯一权威）

> 语义值：`→` 迁改名；`= 不变` 名称无品牌、保持原样；`不迁` 随裁剪删除、新世界无对应物。生产环境变量一律单名 `SLTERM_*`,pebrel 的 `NEBULA_*`/`PEBREL_*` 双名兼容层整体不迁（无历史用户）。

**A. crate / package / 目录**

| 旧名（pebrel) | 新名（slTerminal) | 备注 |
| --- | --- | --- |
| 目录 `nebula_app`,package `nebula` | `slterm_app`(package `slterm`) | bin 见 B-1 |
| package `nebula_terminal` | `slterm_terminal` | |
| package `nebula-split` | `slterm-split` | 零生产依赖契约保留 |
| package `nebula-settings` | `slterm-settings` | 持久化介质改 JSON(eg 06) |
| 目录 `nebula-completions`,package `pebrel-completions` | `slterm_completions`（目录同） | |
| package `nebula_hook` | `slterm_hook` | |
| package `nebula_config` | 并入 `slterm_settings` 内部模块 | B.1 已裁，不独立成 crate |
| package `nebula_config_derive` | 不迁 | proc-macro 无独立载体，derive 按需手实现 |
| package `nebula_gpui` | 不迁 | lab 实验场 |
| `mobile/link`(package `pebrel-mobile-link`) | 不迁 | 随 mobile/ 整删 |
| `third_party/winit-0.30.13` 路径补丁 | 不迁 | legacy 壳 backport |
| `[patch.crates-io] x11-clipboard` | 不迁 | Linux-only |
| 依赖 `mlua` | 不迁 | Lua 配置栈砍 |

**B. 二进制 / CLI**

| 旧名 | 新名 | 备注 |
| --- | --- | --- |
| B-1 bin `pebrel`(nebula_app) | bin `slterm` | clap `bin_name`;CLI 资源+动词归 04 篇 |
| B-2 bin `pebrel-hook`(nebula_hook) | bin `slterm-hook` | |
| B-3 `pebrel setup-ai` 等子命令面 | `slterm setup-ai` 形态保留归 03 篇 | 子命令名无品牌，= 不变 |
| B-4 `docs/skills/pebrel-runtime/`(SKILL.md 自发现目录） | `docs/skills/slterm-runtime/` | 归 04 篇 |

**C. 环境变量（生产契约；测试/QA 变量不逐条迁，归各篇按域新立 `SLTERM_*`)**

| 旧名 | 新名 | 备注 |
| --- | --- | --- |
| `PEBREL_CONFIG_DIR`（别名 `NEBULA_CONFIG_DIR`) | `SLTERM_CONFIG_DIR` | 单名，别名层不迁 |
| `PEBREL_RUNTIME_ENDPOINT` | `SLTERM_RUNTIME_ENDPOINT` | 归 04 篇 |
| `PEBREL_NOTIFY_PIPE` / `NEBULA_NOTIFY_PIPE` | `SLTERM_NOTIFY_PIPE` | 归 03 篇；作用域闸门照抄 |
| `PEBREL_PANE_ID` / `NEBULA_PANE_ID` | `SLTERM_PANE_ID` | per-pane 环境归 03 篇 |
| `PEBREL_PANE_REMOTE` / `NEBULA_PANE_REMOTE` | `SLTERM_PANE_REMOTE` | 同上；SSH 语义消亡后字段重估归 03 |
| `PEBREL_HOOK_EXE` / `NEBULA_HOOK_EXE` | `SLTERM_HOOK_EXE` | 归 03 篇 |
| `PEBREL_CLI` / `NEBULA_CLI` | `SLTERM_CLI` | 归 03 篇 |
| `NEBULA_BASH*` / `NEBULA_ZSH*` / `NEBULA_ZDOTDIR*` 等 unix shell 注入族 | 不迁 | unix shell 集成砍 |
| `PEBREL_SSH_*` / `NEBULA_SSH_*` / `NEBULA_WEBDAV_*` / `PEBREL_BACKUP_*` | 不迁 | SSH / 远端备份砍 |
| `PEBREL_*_QA_*` / `PEBREL_TEST_*` / `PEBREL_CURSOR_QA_*` 等 QA 族 | 不逐条迁 | 各篇建测试时按需新立 `SLTERM_TEST_*` |
| `NEBULA_GPUI_CONFIG` / `NEBULA_GPUI_MANIFEST` / `NEBULA_DETACHED_LAUNCH` 等进程内部变量 | 按域重估归 03/04/09 | fork 时逐条判定是否仍有生产者 |

**D. 用户目录与数据文件**

| 旧名 | 新名 | 备注 |
| --- | --- | --- |
| `%APPDATA%\Pebrel`（默认数据目录） | `%APPDATA%\slterm` | 叶子名 = D2 裁决值 `slterm` |
| `pebrel_settings.txt` / `nebula_settings.txt` | `settings.json` | 持久化 JSON 化已定；写通道归 06 篇 |
| `session.json` / `session.crashed.json` | = 不变 | 无品牌前缀 |
| `runtime.port` | = 不变 | 归 04 篇 |
| `pebrel_providers.json` | `providers.json` | eg 10 篇；去品牌化见 D3 |
| `pebrel_history.jsonl` | `history.jsonl` | eg 08 篇；WSL/SSH 变体不迁 |
| `pebrel-theme.json`(ThemeFormat::Pebrel 自有格式） | `slterm-theme.json` | 格式名归 06 篇；pebrel 格式导入兼容归 06 裁定 |
| `pebrel-workspace.json` / `nebula-workspace.json`(workspace 导出） | `slterm-workspace.json` | 归 05 篇 |
| `pebrel.json` / `pebrel.js` / `pebrel.ts`(ai_hook managed marker 族） | `slterm.json` / `slterm.js` / `slterm.ts` | eg 03 篇 config_guard 监视清单同步 |
| 恢复点文件后缀 `.pebrel-recovery` | `.slterm-recovery` | eg 10 篇；去品牌化见 D3 |
| `nebula_settings/src/paths/migration.rs`(Nebula→Pebrel 迁移链） | 不迁 | 无历史用户 |
| `%APPDATA%\Nebula`(legacy 迁移来源） | 不迁 | 同上 |
| `shell-integration/` 目录内 unix 脚本 | 不迁归 03 重建 | Windows-only 注入归 03 |

**E. 协议 / 管道 / 系统标识**

| 旧名 | 新名 | 备注 |
| --- | --- | --- |
| P-1 线协议头 `nebula-hook/1` | `slterm-hook/1` |归 03 篇；两侧同步 |
| P-2 命名管道 `\\.\pipe\pebrel-notify-{pid}` | `\\.\pipe\slterm-notify-{pid}` |归 03 篇 |
| P-3 AUMID `com.pebrel.terminal` | `com.slterminal.terminal` |归 09 篇消费 |
| P-4 Inno `AppName=Pebrel` / `OutputBaseFilename` 品牌位 | `slTerminal` 品牌位 | GUID 归 12 篇；slTerminal 现有 AppId 续用或新立归 12 |
| P-5 品牌串 `Pebrel` / `Nebula` 全 UI 文案 | `slTerminal` | 归 i18n(06 篇） |
| P-6 crate 元数据 homepage/repository(`github.com/Kuddev/pebrel`) | 本仓归 12 篇定 | |
| P-7 线程名 `pebrel-ai-pipe` | `slterm-ai-pipe` |归 03 篇 |
| P-8 开机启动 .lnk 名 / 托盘工具提示等系统面字符串 | slTerminal 品牌归 09 篇 | |
| P-9 凭据管理器 target `Pebrel/AI/<id>` | `Slterm/AI/<id>` |归 10 篇锚定 |

**F. 双名兼容层（pebrel 为历史用户保留的旧名并写）**

| 旧形态 | 裁定 |
| --- | --- |
| `NEBULA_*` ↔ `PEBREL_*` 环境变量并写 | 不迁，单名 `SLTERM_*` |
| `nebula-workspace.json` ↔ `pebrel-workspace.json` 导出双后缀 | 不迁，单后缀 |
| `nebula_history.jsonl` ↔ `pebrel_history.jsonl` 等历史文件双名 | 不迁 |
| legacy marker / 迁移链全族 | 不迁 |

### 总删减清单的落地映射（fork 拷贝时不进本仓的目录）

按 spec 不采纳总表逐条落到 pebrel 路径（拷贝动作的白名单取反）:`mobile/` 整目录；`nebula_app/src/ssh*.rs`、`ssh_session/`、`remote_dirs.rs`、`backup_remote.rs`、`platform/ssh_agent.rs`；`nebula_app/src/macos/`、platform unix 分支；`nebula_app/src/plugins/`(Lua)、`mlua` 依赖；`nebula_app/src/mux.rs`、`daemon.rs` 驻留语义（注意区分：单实例移交保留归 04，驻留砍）;`nebula_app/src/chemistry.rs`、`scientific_corpus.rs`；亮主题全套（LIGHT_ANSI / follow_system_theme / is_light);legacy 渲染路径（`display/` 旧壳部分、`renderer/`、`product_renderer.rs`、`product_input.rs`、legacy facade 分支、`third_party/winit` patch、window_transition、`nebula_gpui`);Quick terminal;`packaging/linux`、`packaging/macos`、`packaging/scoop`、MSIX 清单；GitHub workflows 本体（机制本地重生）；安装器附加任务（字体/PATH/右键菜单/WSL 子菜单）与 `installer-migration.iss`;WSL 全家（探测/历史池/路径/补全/启动身份）;广播输入、`tabs` 位置配置化、OS 凭据提示框、`TermTheme.powerline` 八槽、应用图标 25 色板（eg pebrel 特有产品面，不采纳总表）。

### slTerminal 旧栈 M0 全删清单 + 自有资产提取路径

**M0 删除（一次性，不与新 workspace 并存）**:`src/`(React/xterm.js/dockview 前端）、`src-tauri/`(Tauri 后端 + Cargo.lock + tauri.conf.json + capabilities/ + gen/ + icons/ + vendor/ + tests-comctl6.manifest)、`package.json` / `package-lock.json` / `node_modules/` / `vite.config.ts` / `vitest.config.ts` / `vitest.l3.config.ts` / `tsconfig*.json` / `eslint.config.js` / `knip.*` / `index.html` / `public/` / `dist/` / `coverage/` / `test/` / `e2e-tests/`、`scripts/` 下 JS 族（check-ts7-trigger / gen-katex-inline / sync-startup-colors)、`.github/`(Tauri 时代 workflows，若存在）、`src-tauri/target/` 构建残留。保留：`.claude/`(CLAUDE.md 族重写归 M11;package.ps1 归 12 篇改造）、`docs/`(pebrel-refactor + pebrel-design 为临时稿，M11 删）、`Microsoft/`(Win10 ConPTY NuGet 捆绑,eg 02 篇）、根 `slterminal-projects.json`（工作区项目数据归 05 篇提取裁决，见开放问题）、`README.md`（归 12 篇重写）。

**自有资产提取（`git show HEAD:<path>` 逐一取之，按归篇落地）**:
- `src-tauri/src/pty/`(DA1/DSR 纯函数族、ConPTY flags 能力矩阵、shell 白名单深检、pwsh EncodedCommand B17、SPAWN_LOCK、Job Object、管道） → 归 02 篇，并入 `slterm_terminal` Windows adapter;
- `src-tauri/src/settings.rs`（原子写 NamedTempFile persist + rename、.bak、SETTINGS_SAVE_LOCK、损坏三态）→ eg 06 篇写通道底座；
- `src-tauri/src/plan_balance/`、`hooks/`（审计三件套 SEC-12/13/17)→ eg 10 篇；
- `src-tauri/src/git/`、`notify/`、`preview.rs`、`fs/`(CRLF 检测）→ eg 07 篇；`projects.rs` eg 05 篇；
- `src-tauri/src/agent_dirs.rs`、`agent_history/` eg 03 篇；`background_tasks/` eg 09 篇；`error.rs` 的错误分类形态归本篇锚点 `AppError` 演化参考；`home.rs` / `app_dir.rs`归 01 消费（home_dir 语义已被 pebrel dirs.rs 覆盖，提取时择优）。

提取纪律：先 M0 删旧栈，后从 git 历史提取——禁止「边删边留」造成双栈并存；每个提取动作在对应分片篇的测试点清单里登记防回归用例。

### 门禁三件套本地落地（无 CI 形态的改造）

pebrel 的门禁挂在 GitHub Actions(`.github/workflows/architecture.yml` 的 range 模式 + release.yml 全量）;slTerminal 无 CI，改造为三层本地执行：

1. **git hooks（强制层）**:`.githooks/pre-commit` → `python3 scripts/check_architecture.py` + `python3 scripts/check_brand_regress.py staged`;`.githooks/commit-msg` → `python3 scripts/check_brand_regress.py message "$1"`;`.githooks/pre-push` → `python3 scripts/check_brand_regress.py push`。一次性启用指令 `git config core.hooksPath .githooks` 写入根 CLAUDE.md 命令节（hooks 目录入 git,.hook 文件自带 python 命令名探测：依次试 `python3` / `python` / `py -3`，命中即用）。
2. **CLAUDE.md 登记层（习惯层）**：根 CLAUDE.md 静态检查门禁节照 spec 总表「测试」行扩写：cargo test / clippy / fmt / `check_architecture.py` / `check_brand_regress.py` 五件为提交前固定动作。
3. **门禁自测四件（可信层）**:`scripts/tests/test_architecture_budgets.py`、`test_architecture_dependencies.py`、`test_architecture_governance.py`、`test_brand_regress.py`（前三个名字照抄 pebrel，第四个由 `test_prohibited_names.py` 骨架改造）——门禁脚本自身可被测试，是「门禁不许静默放宽」的机械保障；governance 件的 note 九段结构 / 文档链接有效性 / 行数上限检查照抄。

`--base` 棘轮在无 CI 时的用法：本地功能分支合入主干前，`check_architecture.py --base main` 等价于 pebrel 的 PR 检查。

### LICENSE 切换与合规族落位

- `LICENSE`:MIT → GPL-3.0 全文（pebrel `LICENSE` 照抄）。因果链：pebrel 上游 GPL-3.0-or-later,fork 派生属「衍生作品」，继续以 MIT 发布即违约；切换是合规义务而非偏好。slTerminal 此前无外部贡献者（个人项目），再许可无第三方授权障碍；git 历史中的 MIT 时期文件保留历史原貌，新头新改一律 GPL。
- `THIRD-PARTY-NOTICES`：结构照抄，内容按 slTerminal 实际依赖树裁剪归 12 篇（Lua/mlua 条目随 Lua 砍删除；新增项随依赖树生成）。
- `licenses/`：目录照抄归仓，其中数学字体许可证归 08 篇消费确认（内嵌拉丁现代数学字体的分发义务）。
- 合规红线（继承 SEC-18)：真实凭据仍禁入任何 git 追踪文件，GPL 切换不改变凭据纪律。

### 注释治理惯例落位

根 Cargo.toml 的 GPUI 注释块、profile 特判注释、依赖 pin 理由注释，密度惯例照抄 pebrel：每处 pin / patch / profile 特判自带原因、影响面、撤销条件三元组，不留裸 magic；改动 profile 体积相关数值必须重跑体积钉测试（归 12 篇）。

## 测试点清单

| 测试 | 层级 | 机制 |
| --- | --- | --- |
| 门禁自测四件（budgets / dependencies / governance / brand) | python unittest(`scripts/tests/test_*.py`) | 移植 pebrel 对应用例，词表/根目录改造 |
| `budgets.Budget.parse` 非法输入族（重复 limit、越界豁免、通配、重叠 root) | 同上 | 移植 |
| 棘轮语义：豁免只减不增、文件缩容报错销豁免、删文件销额度 | 同上 | 移植 |
| 依赖门禁：core→app 生产边拒绝、renderer 入 core 拒绝、dev 环白名单、未知成员/分类全等 | 同上 | 移植 |
| workspace 全等：根 Cargo.toml members 与 dependencies.toml crates 键集一致（隐含于 dependencies.check) | 同上 | 照抄 |
| 禁名门禁四模式正/反例（staged 新增行命中、message 命中、豁免路径放行、merge combined diff) | 同上 | 改造 pebrel test_prohibited_names |
| `settings_dir` 优先级：`SLTERM_CONFIG_DIR` 优先、空串忽略、默认 `%APPDATA%\slterm` | Rust `slterm_settings` 内嵌测试 | 移植 pebrel `new_override_precedes_legacy_alias_and_empty_values_are_ignored` 去别名版 |
| `data_dir` Windows 落点命名测试（`windows_path_uses_slterm_layout`) | Rust `slterm_app` 内嵌测试 | 移植 pebrel `windows_path_uses_pebrel_layout` |
| 新代码树零旧名：全树 grep `nebula` / `pebrel` 仅命中豁免清单 | 脚本断言（可并入 check_architecture report) | 新建，M1 各 crate 迁入后逐 crate 通过 |
| toolchain 钉版一致：rust-toolchain channel == workspace.package rust-version | 脚本断言归 governance 件 | 新建 |
| 旧栈删除完整性：M0 删除路径清单逐项不存在 | 脚本断言（一次性，归 M0 出口脚本） | 新建 |
| LICENSE 合规：LICENSE 首行特征串为 GPL、THIRD-PARTY-NOTICES 存在、licenses/ 非空 | 脚本断言归 governance 件 | 新建 |

## 阶段归属与出口标准

本片跨 M0 / M1，细分步与可机验出口（对齐 00-roadmap，不冲突）:

**M0.1 删旧栈**：出口 = M0 删除清单逐项 `Test-Path` 为假 + `cargo metadata` 无 Tauri 残留 + git 工作树干净。

**M0.2 钉版落骨架**：建 rust-toolchain.toml、根 Cargo.toml(workspace 成员为空壳 crate 占位或仅骨架 workspace、GPUI 四依赖钉版 + 注释块、profile 结构）、各空壳 crate Cargo.toml。出口 = `cargo check` 过；`rustc -V` = 钉版；`cargo metadata` members 与 dependencies.toml 键集全等（空壳期即强制，防骨架漂移）。

**M0.3 门禁三件套**:`architecture/file-budgets.txt`（零豁免初版）、`architecture/dependencies.toml`、`scripts/architecture/{budgets,dependencies}.py`、`scripts/check_architecture.py`、`scripts/check_brand_regress.py` + 自测四件 + `.githooks/` 三钩。出口 = 自测四件全绿；`python3 scripts/check_architecture.py` 零 ERROR;hooks 启用指令写入 CLAUDE.md。

**M0.4 LICENSE 合规族**:LICENSE(GPL-3.0)、THIRD-PARTY-NOTICES、licenses/。出口 = 合规断言测试过。

**M1 内归本片的事项——`slterm_config` 并入 settings**(B.1 已裁并入 settings 单 crate)：诊断薄层以 `slterm_settings` 内部模块形态落位，`capture_diagnostics` / `SerdeReplace` 移植用例全绿，settings 仍满足零生产依赖契约；dependencies.toml / file-budgets roots / members 均按 6 crate 形态开户。其余 M1 crate 迁入序归 00-roadmap（不变）。

各步出口只认机验项；M0 全程无可运行产品，符合过渡期不可用清单。

## 待沉淀决策

> [待沉淀] **D1 · `slterm_config` ± derive 去留**。真实权衡：保留双 crate = 诊断/宽容解析抽象有独立演化面，代价是 proc-macro 编译成本与「无第二个实现」的抽象税；并入 settings = 零抽象税，代价是 settings 的零生产依赖契约下诊断模型只能内嵌（仍可行，纯 serde 逻辑）。难逆点：下游（settings/terminal/app）消费面一旦铺开，再合并/拆分都是跨篇改动。截止：M1 settings 首迁时定，决策记录入 adr.md。

> [待沉淀] **D2 · 数据目录叶子名 `slTerminal` vs `slterm`**。真实权衡：产品名是 slTerminal(`%APPDATA%\slTerminal` 用户可辨识），而 crate/env 前缀统一 slterm(`%APPDATA%\slterm` 全标识符一致）。难逆点：目录发布后迁移即数据搬迁义务（pebrel 自身 Nebula→Pebrel 迁移链即前车之鉴）。意外因素：slTerminal 有 0.2.0 历史用户，旧版 settings.json 在 exe 同级（便携化），新布局落地涉及旧数据去向归 06 篇。截止：M0.2 骨架期定（映射表 D 节默认值按 `slTerminal` 填，可被本决策推翻）。

> [待沉淀] **D3 · 数据文件命名去品牌化程度**。真实权衡：`providers.json` / `history.jsonl` / `settings.json` 去品牌 = 未来再改名零成本（本次 nebula→pebrel→slterm 三连改名的直接教训）；带品牌 = 用户目录里可辨识来源。难逆点：文件名随发布固化，届时改 = 迁移链。注意纪律冲突：pebrel 的迁移链因「有历史用户」而存在，slTerminal 对 pebrel 数据无兼容义务（全新 fork)，但对自身未来版本有。截止：M1 providers/history 迁入前定。

> [待沉淀] **D4 · 禁名门禁扫描范围**。真实权衡：仅新增行（pebrel 形态）= 与 git 工作流咬合、无存量噪音，但存量旧名要靠「一次清零」承诺兜底；全树常挂 = 存量永不腐化，但要求 fork 拷贝当日全树零旧名——而 GPUI 注释块、THIRD-PARTY-NOTICES 等合法来源必须逐个精确豁免，豁免表本身的维护成本是真实代价。难逆点：若先选「仅新增行」且存量未清零，日后想升全树需补一次大扫除，债务随时间增长。意外因素：临时稿目录 docs/pebrel-design|refactor 合法含旧名至 M11。截止：M0.3 门禁落地时定（默认：新增行模式 + M1 全量迁入完成后做一次全树清零 + 清零后切换全树常挂；该默认本身即折中，值得记录）。

## 开放问题

1. **python 命令名**：本机 Windows 无 CI，门禁脚本与 hooks 的 python 探测序（`python3` → `python` → `py -3`）是否可接受？默认按此序探测并写入 hooks；若本机只有 `py`，首次 pre-commit 将静默通过不了——M0.3 出口脚本需实测一次真实提交验证。
2. **竞品名禁表**:pebrel 的 `check_prohibited_names.py` 词表防的是自家品牌被竞品名污染，slTerminal 无此诉求，默认不迁（禁名门禁只管 nebula/pebrel 回流）。若用户希望保留竞品名禁表（如防 AI 生成时引入 `ghostty` 等参考名），在本篇门禁设计上加一张词表即可，无结构成本。
3. **GPUI 基线跟随节奏**:pebrel 后续若升 Kuddev rev，slTerminal 是否一律跟随（锁同 rev 集，防类型双源）？默认锁同集，跟随动作走 00-roadmap 增量评审。
4. **`slterminal-projects.json`**（仓库根的工作区项目数据文件）：提取归 05 篇（projects 领域）还是随旧栈删除？默认提取——它是用户工作区数据，删除不可逆。
5. **Inno AppId GUID**：续用 slTerminal 现有 GUID 还是新立？归 12 篇，但影响改名的品牌位一致性，提前登记。
