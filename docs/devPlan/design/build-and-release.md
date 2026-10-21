# 构建与发布

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 拥有目标态 workspace 布局、二进制与包名、安装器与便携包、更新交接、架构门禁与体积预算重登记；发布链保持单一入口、同源构建与可复现核验。

### 决策与红线

- **新 crate 布局按层收敛**：新增五个领域 core crate（`slterm_workspace`、`slterm_sidebar`、`slterm_agents`、`slterm_hooks`、`slterm_runtime_protocol`）；命令/焦点、持久化协调器与 i18n 目录留在 app，不新增 `slterm_persist`/`slterm_commands`/`slterm_i18n`；终端、分屏、设置、补全继续作为 core 引擎；只有应用壳依赖 GPUI 与平台集成。原因：旧壳的 workspace/tab/pane/dock/session 整体废弃，领域模型必须脱离 UI 可测。边界：不保留旧模型兼容层，不迁移旧数据。
- **包名与二进制名稳定**：应用包 `slterm`、二进制 `slterm.exe`；Hook 包 `slterm_hook`、二进制 `slterm-hook.exe`；产品显示名 `slTerminal`。原因：安装器、更新链、单实例与 AUMID 已按这些名称形成外部契约。边界：目录布局可以重排，包名与资产名不变。
- **单一发布入口**：正式发布只走 `scripts/release.ps1 -Version <X.Y.Z>`，依次共享构建、安装器、便携 zip、钉测试、发布核验。原因：两个产物必须同代同 feature 图。边界：正式路径禁止跳过构建或复用旧产物；彩排必须显式标注。
- **清单即合同**：包内文件集唯一真值源是 `scripts/build-windows-product.ps1` 的 `PackageEntries`；安装器 `[Files]`、便携 zip staging 与钉测试必须同步。原因：包内文件集是外部可见合同。边界：新增/删除文件必须同时修改三处。
- **更新包不可执行未验证内容**：名称/架构/版本/官方 URL 前缀四重匹配、SHA-256、长度、PE 头与 512MiB 上限全部通过后才原子替换；commit 是唯一安装授权。原因：下载链是远程输入，必须默认拒绝。边界：便携版不自动安装，提示走下载页手动替换；外部托管渠道不参与自更新。
- **门禁常挂**：格式、clippy、测试、架构、文档策略、品牌回归、打包钉测试与发布核验全部通过才可发布。原因：目标态重建允许大改，但不允许静默降低质量门。边界：新增 crate 必须先登记依赖策略与行预算，否则架构门禁失败。

### 目标 workspace 布局

```text
slterm_workspace/          core   项目/布局/面板模型、ID、封闭枚举、纯转换、持久化 DTO
slterm_sidebar/            core   侧栏布局、文件树/Commit/Agent 文件纯模型
slterm_agents/             core   AgentKind 能力表、会话元数据、hook 事件归一化
slterm_hooks/              core   Hooks 事件/handler 目录、schema 校验
slterm_runtime_protocol/   core   Runtime v2 信封、线缆 ResourceId、请求/响应 schema
slterm_settings/           core   设置值模型、键注册表、主题色表、路径权威
slterm_split/              core   面板组几何树、比例、导航纯函数
slterm_terminal/           core   PTY、VT、输入编码、选择、渲染快照
slterm_completions/        core   补全匹配引擎（范围外增强，后续迁移）
slterm_app/                app    GPUI 壳、平台集成、面板 UI、服务编排、持久化协调器、Runtime 服务器
slterm_hook/               hook   AI CLI hook 转发（零生产依赖）
```

依赖方向：

```text
slterm_app (application)
  ├─ slterm_workspace
  ├─ slterm_sidebar
  ├─ slterm_agents
  ├─ slterm_hooks
  ├─ slterm_runtime_protocol
  ├─ slterm_terminal
  ├─ slterm_settings
  ├─ slterm_split
  └─ slterm_completions

slterm_sidebar  → slterm_workspace / slterm_split
slterm_hooks    → slterm_agents
slterm_workspace→ slterm_split
slterm_agents / slterm_runtime_protocol → 无生产依赖
slterm_hook     → 无生产依赖
```

- 渲染框架包（`gpui`、`gpui_platform`、`gpui-component`、`winit`、`glutin`、`crossfont` 等）只允许应用层依赖；core 与 hook 生产依赖不得出现。
- `slterm_workspace` 不引用 GPUI、文件系统或网络；项目/布局/面板的 ID、枚举、状态机与纯转换可独立测试。
- `slterm_sidebar` 只拥有侧栏布局、文件树/Commit/Agent 文件的纯模型与状态迁移；文件系统执行、Git 进程与 GPUI 留在 app。
- `slterm_agents` 只拥有 AgentKind 能力表、会话元数据与 hook 事件归一化；不启动进程、不读文件、不访问网络。
- `slterm_hooks` 只拥有事件/handler 目录与 schema 校验；文件读写与 CLI 启动留在 app 服务层。
- `slterm_runtime_protocol` 只拥有协议类型与线缆 `ResourceId`；`ResourceId` 是不透明字符串，自带解析/格式化/错误，不依赖 `slterm_workspace`；该 crate 保持零 workspace 生产依赖。服务器、传输与鉴权在 app。
- 持久化协调器、命令/焦点框架、i18n 目录与平台适配器留在 app；不新增 `slterm_persist`/`slterm_commands`/`slterm_i18n` crate。
- `slterm_terminal`、`slterm_settings`、`slterm_split`、`slterm_completions` 保留既有职责，但对外接口按目标模型调整；旧 `workspace`/`tab`/`pane`/`dock`/`session` 模块随旧壳删除。
- 每个 crate 使用显式 `[[test]] name = "lib_tests"` 与 `[lib] test = false`；定向测试一律 `cargo test -p <crate> --test lib_tests <filter> -- --test-threads=1`。
- feature 图保留 `gpui-test-support` 与 `update-test-source`；删除旧的 `legacy-attach` 兼容 feature，单实例移交只保留版本化载荷。测试专用 feature 不得进入生产构建。
- 新 crate 目录必须同步创建路径 `CLAUDE.md`，登记该域决策与验收方式；旧目录 `CLAUDE.md` 随旧代码删除。

### 依赖钉死与基线验收

- GPUI 与 gpui-component 使用根 `Cargo.toml` 钉死的 fork rev；升级必须单独评审，升级前跑全量门禁与体积基线。
- Tree-sitter grammar 每语言一个 crate；编辑器面板里程碑前完成选型、许可与体积登记。新增依赖同步登记 `architecture/dependencies.toml` 与 `architecture/file-budgets.txt`；超体积预算时裁剪语言集。
- 构建与打包基线里程碑的验收落点：workspace 成员、依赖策略键集、`PackageEntries`、安装器/便携包 staging 与体积基线；退出条件为架构门禁全绿、包清单三处一致、体积测试有可复现基线。

### 二进制、包名与资产

| 对象 | 目标值 | 说明 |
|---|---|---|
| 应用包/二进制 | `slterm` / `slterm.exe` | 单一 GUI 进程，console 子系统形态按启动链约定 |
| Hook 包/二进制 | `slterm_hook` / `slterm-hook.exe` | 随包放在 `runtime/`，零生产依赖 |
| 产品显示名 | `slTerminal` | 安装器、快捷方式、窗口标题、关于页 |
| 安装器文件名 | `slTerminal-v<version>-windows-x64-setup.exe` | 资产名是更新链契约 |
| 便携包文件名 | `slTerminal-v<version>-windows-x64.zip` | 与安装器同源构建 |
| AppId | 既有生产 AppId | 一经钉死永不变更；验收夹具使用独立 AppId |
| AUMID | `com.slterminal.terminal` | 系统通知归因与任务栏归组 |
| 开机启动快捷方式 | `slTerminal.lnk` | 用户 Startup 文件夹；安装器与设置页同源 |

- 版本号由发布脚本传入；版本线延续既有更新链规则，低于首个安装器基版本的旧安装视为需手动重装。
- 包内资产：应用图标 PNG/ICO、许可文件、第三方声明、Runtime API v2 文档与 schema、随包 skill、CHANGELOG；不再携带 v1 schema 与旧版 skill 文案。
- 包内文件集、安装器脚本、便携 zip staging 与钉测试必须一致；资产路径变更时同步更新。

### 安装器与便携包

- 安装器使用 Inno Setup，用户级安装（`PrivilegesRequired=lowest`），最低 Windows 版本 `10.0.17763`；不写 HKLM，不需要管理员。
- 安装目录默认在用户 Programs；`UsePreviousAppDir=yes` 支持续装；卸载项、App Paths、开始菜单、桌面快捷方式与开机启动快捷方式按任务创建。
- 安装器语言包含英文与简体中文；中文翻译文件固定上游提交与 SHA-256，缺失或哈希不符拒绝构建。
- 便携包与安装器使用同一构建目录、同一 feature 图与同一文件清单；便携包解压后文件集必须与 `PackageEntries` 全等。
- 便携版没有 Inno 卸载器，更新链拒绝自动安装并提示从下载页替换；分发标记文件 `slterm-distribution` 存在且无法识别时按外部托管处理，拒绝自更新。
- 体积预算：release profile 冷代码 `opt-level = "s"`、thin LTO、单 codegen unit、strip；热路径（渲染、塑形、布局、终端核心、GPUI、应用壳）钉回 O3。预算值以实测 + 15% 裕度钉在打包钉测试；超预算即红，收紧永远合法，放宽须在提交信息说明原因。
- 改动 profile 体积相关数值、热路径钉包清单或包内文件集，必须重跑打包钉测试与体积测试。

### 更新检查与交接

```rust
pub struct UpdateAsset {
    pub version: String,
    pub name: String,
    pub url: String,
    pub size: Option<u64>,
    pub digest: Option<String>,       // SHA-256 小写 hex；缺失则拒绝下载
}

pub struct UpdatePlan {
    pub schema: u16,
    pub asset: UpdateAsset,
    pub transaction: String,
    pub executable: PathBuf,
    pub installation: PathBuf,
    pub config_directory: PathBuf,
    pub installer: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub version: String,
    pub original_version: String,
    pub guard_path: PathBuf,
    pub participants: Vec<Participant>,
}

pub struct Participant { pub pid: u32, pub created: u64 }
```

检查：

- 启动检查是延迟执行的后台任务；手动检查可随时触发。断网、坏 JSON、限流只记录，不弹错、不阻塞界面。
- 版本比较按点分段数值，拒绝降级与等版本；资产必须名称、架构、版本与官方 URL 前缀四重精确匹配；`digest` 优先，release 正文 checksum 回退。
- release JSON 正文上限 2MiB；REST 403/429 才允许官方 release 页面的有界回退；不使用 token、不猜包 URL。

下载：

- 流式写入同目录 `.part` 文件；长度、PE `MZ`、SHA-256、512MiB 上限全部通过后原子替换。
- 每个下载任务持有单调 generation；取消或换版本后旧任务结果失配失效；取消不需要旧任务句柄。
- 下载状态与会话持久化；启动水合可在无网时恢复就绪/失败态。

安装交接：

1. 准备：校验安装目录存在 `unins000.exe`；生成事务目录与 `plan.json`；启动 PowerShell helper（固定 System32 路径、无窗口、无配置、非交互），等待 helper 就绪。
2. 授权：用户确认后原子写 `commit.json`；这是唯一安装授权。helper 在该进程退出后运行安装器。
3. 执行：helper 等待参与者进程退出（PID + 创建时间双因子），运行安装器，写入 `result.json`；失败时回滚并写失败证据。
4. 恢复：下次启动读取结果；版本匹配成功或回滚后原版本匹配才发 restore ticket；最多 3 次恢复尝试，`restored.json` 防重放。
5. 孤儿事务：无 `commit.json` 的 plan/ready 按取消安全收尾，绝不因文件存在而授权安装。

- 安装期间旁置守卫锁阻止同一二进制被替换；事务目录与日志不写入用户项目目录。
- 更新链不执行网络 Git 操作、不修改系统代理、不请求管理员权限；外部托管渠道只提示对应包管理器命令。
- 更新包校验失败、下载失败或安装失败均可见；失败不改变当前安装，不产生半安装状态。当前链以 GitHub release 元数据中的 SHA-256 为信任依据；有 Authenticode 证书时签名安装器与二进制，并在 handoff 前增加独立签名验证步骤；无证书时在发布清单登记残余风险，摘要校验不放宽。

### 架构门禁与预算重登记

新增或重排 crate 时必须同步：

| 位置 | 变更 |
|---|---|
| 根 `Cargo.toml` | workspace 成员与包名一致；无通配成员 |
| `architecture/dependencies.toml` | 每个成员一条规则：layer、精确依赖列表、零依赖标记；键集与 workspace 成员全等 |
| `architecture/file-budgets.txt` | 新 crate 根登记行预算；删除旧模型后重基线，禁止新增债务 |
| `scripts/build-windows-product.ps1` | `WorkspaceMembers` 与 `PackageEntries` 同步；构建包名与二进制名 |
| `scripts/installer.iss` | `[Files]` 与包内清单一致；AppId、AUMID、最低系统版本不变 |
| 打包钉测试 | 文件清单、新鲜度、profile 钉值、体积预算同步 |
| 路径 `CLAUDE.md` | 新模块目录创建；旧目录删除 |

- 依赖方向门禁：core 不得依赖应用层或渲染框架；hook 生产依赖为空；应用层是唯一可依赖渲染框架的成员。
- 行预算门禁：删除旧 `workspace`/`tab`/`pane`/`dock`/`session` 后重跑 `python scripts/check_architecture.py`，以新基线替换旧债；不得用豁免掩盖新增债务。
- 体积门禁：新 crate 默认作为冷代码参与 `opt-level = "s"`；只有实测热路径才加入钉包清单。删除旧模型后重新实测安装器与便携 zip，按实测 + 15% 重钉预算。
- 发布前门禁：`cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets`；`cargo clippy -p slterm --all-targets --features gpui-test-support`；`cargo test --workspace -- --test-threads=1`；`python scripts/check_architecture.py`；`python scripts/check_doc_policy.py full`；`python scripts/check_brand_regress.py staged`；打包钉测试与 `verify-release`。
- 品牌门禁：禁名词表只允许 `slTerminal`/`slterm`；旧品牌名不得回流到二进制、资产、文档或安装器。
- 合规四落点：仓库根许可证全文、workspace/成员 `license`、安装器许可页、应用内关于页；第三方声明随包。

### 验收与测试

- 布局门禁：新 workspace 成员、依赖策略键集、行预算根三者全等；core 与 hook 生产依赖无渲染框架；每个 crate 有显式 `lib_tests`。
- 构建：`scripts/build-windows-product.ps1` 从隔离目标目录构建两个二进制；安装器与便携包使用同一产物；新鲜度判据通过。
- 包合同：安装器 `[Files]`、`PackageEntries`、便携 zip staging 全等；资产名集合、PE 魔数、1MiB 下限、SHA256SUMS 复算一致。
- 体积：release profile 钉值与热路径清单与脚本一致；体积测试通过；预算收紧/放宽有记录。
- 更新：假 release 服务器覆盖检查、四重匹配、摘要缺失拒绝、降级拒绝、下载中断、摘要不符、PE 头不符、超限、取消 generation、恢复票据与孤儿事务清理。
- 安装交接：替身 helper 覆盖准备/授权/失败回滚/清理失败；真实安装器与静默安装为人工核验或条件跳过。
- 发布：`scripts/release.ps1 -Version <X.Y.Z>` 全链通过；`verify-release` 独立复跑；发布清单人工核验完成。
