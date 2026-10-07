# 12 文档治理与打包发布详细设计

> pebrel-design 分片 12/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/12-docs-packaging.md`(spec 分片);骨架:`docs/pebrel-design/00-roadmap.md`(M11)。
>
> 边界分工:打包链本体与发布面归本篇;体积预算钉测试与安装器自测的**测试形态**归 11 篇(本篇只给被钉的契约数值);自动更新运行时链归 09 篇(本篇只给发布坐标与资产命名合同);ConPTY 内嵌形态归 02 篇(本篇只给 manifest 边界);SKILL 资产内容归 04/08 篇(本篇只给随包归位)。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e`),禁行号。

## 目标形态

终态三面,全部本地可跑(无 CI)。

### 文档治理终态(双轨 + 渐进披露)

| 文件面 | 职责 | 形态来源 |
|---|---|---|
| 根 `CLAUDE.md` | 全局规则 + 架构节(ownership 双列聚合表)+ 文档规范节 | slTerminal 现状保留,M11 按 GPUI 时代重写 |
| 各 crate / 目录 `CLAUDE.md` | 模块细节真值;最近路径优先、细节不回填根文件 | slTerminal 渐进披露机制保留,按新 workspace 重建 |
| `.claude/adr.md` | 当前生效决策的当前形态(原位收敛 + 索引表 + 沿革行) | slTerminal 现状原样保留 |
| `architecture/notes/` | 决策成因档案与事故因果(不可改写、Supersedes 链、Revisit when) | pebrel `architecture/notes/AGENTS.md` 规则照抄,载体改名 CLAUDE.md |
| `CONTEXT.md` | 纯术语表 | slTerminal 现状原样保留 |
| `.claude/test-exemptions.md` | 豁免登记单表真值源 | slTerminal 现状保留,重审归 11 篇 |
| `CHANGELOG.md` + `release-notes/` | 双语发布说明(进行中 + 按版本) | pebrel `docs/release-notes/` 形态照抄,落位改根级 |

写决策的默认问句:「五年后还会被翻出来问责吗」——会则 note;仍是当前规则则 adr.md;能从代码自证则不写。

### 打包终态(单入口 + 清单即合同)

`scripts/` 四件 + 一件核验:

| 脚本 | 职责 |
|---|---|
| `build-windows-product.ps1` | 共享显式构建入口(CARGO_TARGET_DIR 隔离,安装器与 zip 同源同 feature 图) |
| `build-installer.ps1` | 新鲜度核验链 + 版本回读 + isl pin + ISCC 探测 → Inno setup.exe(主形态) |
| `package-release.ps1` | staging 清单全等校验 + 安全清理边界 + 原子替换 → 便携 zip(副形态,共享 manifest 真值源) |
| `installer.iss` | 全参数化安装器定义(脚本零硬编码版本) |
| `verify-release.ps1` | 发布核验:资产名集合 + 尺寸下限 + PE 魔数 + SHA256SUMS 生成(吸收 pebrel release 脚本的校验纯逻辑) |

发布链:完成双语 CHANGELOG → 共享构建 → 新鲜度核验 → 安装器 + zip → 钉测试(归 11)→ verify-release → 人工核验清单 → 本机与另一台 PC 双点验收,结论落 notes。

### 合规终态(GPL-3.0 四落点 + 随包合规族)

四落点:仓库根 `LICENSE` 全文 / workspace 根 `Cargo.toml` license 字段 / 安装器许可页 / 应用内关于页。随包合规族:`licenses/`(LICENSE + THIRD-PARTY-NOTICES + 组件专属许可证全文)进 manifest 必含项,双语 CHANGELOG 随包内置。

## 边界与不变更项

1. **产品定位七条**(00-roadmap 跨领域不变量):仅 Win10/11 原生;单窗口单实例;仅暗色;GPU 加速;复制 = Ctrl+Shift+C;默认 shell pwsh → powershell → cmd;27 家 AI CLI 一等。安装器 `MinVersion=10.0.17763` 与定位咬合。
2. **旧世界词只准出现在消亡/映射/来源语境**:WebView、IPC、Tauri 命令、Dockview、xterm.js、vitest、wdio、`.claude/package.ps1` 旧链路。旧链路的唯一归宿是「被 scripts/ 体系整体取代」,不做兼容并存。
3. **不建清单**:GitHub workflows 本体、CI 编排、GitHub Release 编排脚本、MSIX、Scoop、Linux/macOS 打包、arm64 构建(iss 的 arm64 参数化形态照抄保留为后续可选项)、installer-migration.iss(slTerminal 无改名前身的安装基)、安装器附加任务(字体安装 / PATH 注入 / 右键菜单 / WSL 子菜单)。
4. **本篇不写的面**(各归其篇,只引用):

   | 面 | 归属篇 |
   |---|---|
   | 体积预算钉测试、安装器自测的测试形态 | 11 |
   | 自动更新下载/校验/handoff 运行时链 | 09 |
   | ConPTY `include_bytes!` 嵌入与提取 | 02 |
   | SKILL.md 内容与投放契约 | 04/08 |
   | 门禁三件套脚本本体与 hooks | 01 |
   | 豁免表重审 | 11 |

5. **docs/ 临时稿约束**:`docs/pebrel-refactor/` 与 `docs/pebrel-design/` 为临时稿,M11 删除;一切永久面(文档、资产、脚本)不得落 docs/ 下、不得引用这两目录为终态依据。
6. **绿灯语义**:本篇各阶段出口只认可机验项;「发布可用」以脚本实跑 + 核验清单全勾为准。
7. **引用纪律**:引 pebrel 用符号名 + 文件路径;引本仓源码用符号名;引测试用例名。

## 关键类型与签名

### scripts/ 脚本族参数面(照抄 pebrel 同名件,裁剪点已标)

```powershell
# build-windows-product.ps1(照抄 pebrel 同名,改包/feature 名)
param(
    [ValidateSet('debug', 'release')] [string] $Configuration = 'release',
    [string] $TargetDirectory            # 默认 <repo>\target;隔离 CARGO_TARGET_DIR,
)                                        # 防打死正在运行的实例(os error 5 教训照抄)
# cargo build --locked --timings -p slterm --bin slterm -p slterm_hook --bin slterm-hook
#     --features slterm/gpui-shell [--release]
# feature 图与钉测试、安装器、zip 三方同源(11 篇 feature 表含 gpui-shell)。
```

```powershell
# build-installer.ps1(照抄 pebrel 同名,砍双品牌/legacy 壳判定)
param(
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$')] [string] $Version,
    [ValidateSet('release')] [string] $Configuration = 'release',
    [ValidateSet('x64', 'arm64')] [string] $Architecture = 'x64',   # arm64 形态保留不构建
    [switch] $SkipBuild, [switch] $AllowStale,   # 仅限脚本自测;正式发布一律全新构建
    [switch] $Force, [switch] $ValidateOnly,
    [string] $OutputDirectory, [string] $TargetDirectory, [string] $InnoCompiler
)
# 核验链(顺序即合同):
#   1. -Version 缺省回读 slterm_app/Cargo.toml 的 version(正则同 pebrel)
#   2. -SkipBuild 缺席 → 调 build-windows-product.ps1
#   3. 必填收录文件存在性检查(清单同 installer.iss [Files])
#   4. 陈旧检查:二进制 mtime ≥ 全部成员 crate 与根 Cargo.toml 最新 .rs/.toml mtime
#      (cargo 对未变更目标不重链接,禁以运行开始时刻为基准;判据照抄)
#   5. 版本回读:& slterm.exe --version 输出必须含 $Version(防「装进去的是旧 exe」)
#   6. isl pin:ChineseSimplified.isl 固定上游提交 + SHA256 校验,缺失/校验失败才下载
#   7. ISCC 探测序:$ISCC_PATH → %LOCALAPPDATA%\Programs\Inno Setup 6 → ProgramFiles 候选
#   8. ISCC /D 全参数注入 → 产物存在性 + SHA256 输出
# 消亡项:PackageBrand 双品牌参数(NebulaTerminal/Pebrel)塌缩为单名;
#   `--help 含 --gpui` 的 legacy 壳判定随双壳消亡删除(单壳无此风险面)。
```

```powershell
# package-release.ps1(照抄 pebrel 同名,manifest 换 slterm 面)
param( [string] $Version = 'unreleased', [ValidateSet('release')] [string] $Configuration = 'release',
       [ValidateSet('x64','arm64')] [string] $Architecture = 'x64',
       [switch] $SkipBuild, [switch] $AllowStale, [switch] $Force,
       [string] $OutputDirectory, [string] $TargetDirectory )
# 合同函数(签名与语义照抄):
#   Assert-Manifest([string] $Root, [string[]] $Expected)
#     staging 实际文件集与期望清单 Compare-Object 全等,多一个少一个都红;
#     条目路径禁逃逸 staging 根。
#   Remove-StageSafely([string] $Path)
#     解析后路径必须以 <输出根>\.stage- 为前缀否则拒删。
#   Get-NewestSourceTime([string[]] $Roots) / Assert-FreshBinaries   # 判据同上
# zip 产物流:staging → Assert-Manifest → Compress-Archive 临时 zip
#   → 二次读回 zip 条目与 manifest 全等比对 → Move-Item 原子替换 → SHA256 输出。
```

```powershell
# verify-release.ps1(新建;吸收 pebrel scripts/stable_release.py 的
#   validate_assets/_check_magic/sha256/checksum_text 与 preview_release.py 的
#   MIN_ASSET_SIZE/verify_binary_freshness/validate_version 纯逻辑,收敛为 Windows 单面)
param( [Parameter(Mandatory)] [string] $Version, [string] $Directory = '<repo>\dist' )
# 断言(失败即非零退出):
#   1. 资产名集合全等:{setup.exe, zip} ⊆ 期望模板,多一个少一个都红(生成物 SHA256SUMS 除外)
#   2. 每件 ≥ MIN_ASSET_SIZE(1 MiB,防空资产);.exe 首两字节 MZ(PE 魔数)
#   3. 生成 SHA256SUMS("<hash>  <name>" 逐行)——进 release notes 与 09 篇更新链
#   4. 版本号 strict semver(三数值段)
```

### installer.iss 关键段(裁剪后骨架示例)

```iss
#ifndef AppVersion
  #define AppVersion "0.3.0"          ; 默认值仅为 ISCC 直开兜底,正式发布一律 /D 注入
#endif
#ifndef NumericVersion
  #define NumericVersion "0.3.0.0"
#endif
#ifndef Configuration
  #define Configuration "release"
#endif
#ifndef Architecture
  #define Architecture "x64"          ; arm64 参数化形态照抄保留,构建不做
#endif
#define RepoRoot ".."
#ifndef BuildRoot
  #define BuildRoot RepoRoot + "\target\" + Configuration
#endif

[Setup]
#ifdef AcceptanceFixture               ; 夹具模式照抄 installer-migration.iss 头部:
AppId={{<夹具 GUID>}                   ;   隔离 AppId/注册表键,自测不污染真实卸载项
AppName=slTerminal Update Acceptance
#else
AppId={{<新立 GUID,见待沉淀 D12-1>}
AppName=slTerminal
#endif
AppVersion={#AppVersion}
AppPublisher=LiuShenLan
AppUpdatesURL=https://github.com/LiuShenLan/slTerminal/releases  ; 与 SLTERM_RELEASES_PAGE
                                       ; 同真值,坐标定稿后回填(开放问题 1)
VersionInfoVersion={#NumericVersion}
DefaultDirName={autopf}\slTerminal     ; PrivilegesRequired=lowest 下即
                                       ; %LOCALAPPDATA%\Programs\slTerminal;
                                       ; 叶子名与 01 篇 D2 数据目录同名原则
UsePreviousAppDir=yes
UsePreviousGroup=no
DisableProgramGroupPage=yes
PrivilegesRequired=lowest              ; 用户级安装:写 HKCU、无需管理员,咬合单实例定位
ArchitecturesAllowed=x64compatible
MinVersion=10.0.17763                  ; Win10 1809 下界 = 产品定位下界
LicenseFile={#RepoRoot}\LICENSE        ; 四落点之三:安装向导许可页
SetupIconFile={#RepoRoot}\slterm_app\windows\slterm.ico
UninstallDisplayIcon={app}\slterm.exe
OutputDir={#RepoRoot}\dist
OutputBaseFilename=slTerminal-v{#AppVersion}-windows-{#Architecture}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
SetupLogging=yes
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "{#RepoRoot}\target\installer-tools\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "{cm:AutoStart}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#BuildRoot}\slterm.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BuildRoot}\slterm-hook.exe"; DestDir: "{app}\runtime"; Flags: ignoreversion
Source: "{#RepoRoot}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\README.zh-CN.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\CHANGELOG.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#RepoRoot}\assets\runtime-control-api.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#RepoRoot}\assets\runtime-api-v1.schema.json"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#RepoRoot}\assets\skills\slterm-runtime\SKILL.md"; DestDir: "{app}\skills\slterm-runtime"; Flags: ignoreversion
Source: "{#RepoRoot}\assets\skills\slterm-runtime\agents\openai.yaml"; DestDir: "{app}\skills\slterm-runtime\agents"; Flags: ignoreversion
Source: "{#RepoRoot}\LICENSE"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#RepoRoot}\licenses\LICENSE-LATIN-MODERN-MATH"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#RepoRoot}\THIRD-PARTY-NOTICES"; DestDir: "{app}\licenses"; Flags: ignoreversion

[Icons]
Name: "{group}\slTerminal"; Filename: "{app}\slterm.exe"; WorkingDir: "{%USERPROFILE}"; AppUserModelID: "com.slterminal.terminal"
Name: "{group}\{cm:UninstallProgram}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\slTerminal"; Filename: "{app}\slterm.exe"; WorkingDir: "{%USERPROFILE}"; AppUserModelID: "com.slterminal.terminal"; Tasks: desktopicon
Name: "{userstartup}\slTerminal"; Filename: "{app}\slterm.exe"; WorkingDir: "{%USERPROFILE}"; AppUserModelID: "com.slterminal.terminal"; Tasks: autostart

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\slterm.exe"; ValueType: string; ValueName: ""; ValueData: "{app}\slterm.exe"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\slterm.exe"; ValueType: string; ValueName: "Path"; ValueData: "{app}"

[Run]
Filename: "{app}\slterm.exe"; Description: "{cm:LaunchProgram}"; WorkingDir: "{%USERPROFILE}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; 必须在 Inno 删除 slterm.exe 前让应用自己清理注入各 AI CLI 的 hook(03 篇安装权威),
; 避免用户配置残留指向已删程序。
Filename: "{app}\slterm.exe"; Parameters: "setup-ai --remove"; WorkingDir: "{app}"; RunOnceId: "RemoveSltermAiHooks"; Flags: runhidden skipifdoesntexist
```

裁剪清单(对照 pebrel `scripts/installer.iss`,逐项有裁):字体 `[Tasks] installfont` 与 `FontInstall` 两条 Source(无捆绑字体计划,字体策略归 06 篇);`addtopath` 任务 + `NeedsAddToPath` + `CurUninstallStepChanged` 整段(终端非命令行工具,无需 PATH);右键菜单与 WSL 子菜单 `[Registry]` 族(首版不做,登记后续增强);`#include "installer-migration.iss"` 与 `DefaultDirName={code:DefaultInstallDir}`(无旧安装基,改静态 `{autopf}`);快捷方式 `--gpui` 参数(双壳消亡);`ChangesEnvironment=yes`(无 PATH 变更即无广播对象)。保留件:`desktopicon` / `autostart` 两可选任务(默认不勾)与 App Paths 注册(Win+R 启动便利,无 PATH 污染,卸载自清理)。

### 发布坐标与资产命名合同(09 篇消费,本篇登记)

```rust
// 落位:slterm_app/src/platform/update/releases.rs(模块归 09 篇,常量值归本篇登记;
// 09 篇 update_check 只许 use,禁另立)。
pub(crate) const SLTERM_RELEASES_OWNER: &str = "LiuShenLan";      // 真值待定,开放问题 1
pub(crate) const SLTERM_RELEASES_REPO: &str = "slTerminal";
pub(crate) const SLTERM_RELEASES_API: &str =
    "https://api.github.com/repos/LiuShenLan/slTerminal/releases/latest";
pub(crate) const SLTERM_RELEASES_PAGE: &str =
    "https://github.com/LiuShenLan/slTerminal/releases";
pub(crate) const SLTERM_RELEASES_DOWNLOAD_PREFIX: &str =
    "https://github.com/LiuShenLan/slTerminal/releases/download/";
```

资产命名合同(更新链四重校验的本篇侧真值):

| 资产 | 模板 | 说明 |
|---|---|---|
| 安装器(主) | `slTerminal-v{version}-windows-x64-setup.exe` | 09 篇 `windows_x64_installer_names` 三元组塌缩后的唯一名;无 legacy 别名 |
| 便携包(副) | `slTerminal-v{version}-windows-x64.zip` | 共享 manifest 真值源;更新链中 = 手动更新(handoff 行为归 09 D09-2) |
| 校验单 | `SHA256SUMS` | `<sha256>  <name>` 逐行,verify-release.ps1 生成,进 release notes 与 09 篇 digest 校验 |

pebrel 侧历史名兼容表(`stable_release.py::expected_asset_names` 的旧名递进、`LEGACY_RELEASE_DOWNLOAD_PREFIX`)整体不迁——slTerminal 资产名自首发起单名登记。

### 体积工程 profile 块(01 篇锚结构,本篇定钉包清单)

```toml
# 根 Cargo.toml(结构归 01 篇;热路径清单与预算注释归本篇)
[profile.release]
lto = "thin"                 # thin LTO 保留逐包优化级;依赖单 codegen unit
debug = 0
strip = "symbols"
incremental = false
codegen-units = 1
# 发布包体积预算:安装器 <30MB(初始值,首版实测校准后钉死,见待沉淀 D12-2)。
# 冷代码(协议栈、序列化、CLI 等几百个依赖)按体积编译;渲染/塑形/布局/VT
# 热路径在下面逐个钉回 O3,手感不受影响。改动这里必须重跑
# scripts/tests/package-release.tests.ps1(归 11 篇)。
opt-level = "s"

[profile.release.package]
resvg = { opt-level = 3 }
rustybuzz = { opt-level = 3 }
taffy = { opt-level = 3 }
ttf-parser = { opt-level = 3 }
smol = { opt-level = 3 }
gpui = { opt-level = 3 }
gpui-component = { opt-level = 3 }
slterm_terminal = { opt-level = 3 }
slterm = { opt-level = 3, codegen-units = 16 }   # 主 crate 并行 codegen 同 O3
image = { opt-level = 3 }
```

钉包清单纪律:清单 = 渲染(resvg/image)+ 塑形(rustybuzz/ttf-parser)+ 布局(taffy)+ VT 与终端核心(slterm_terminal)+ 壳(slterm/gpui/gpui-component/smol)——pebrel 同构映射,随架构演进重登记并重跑钉测试;`panic = "abort"` 不在本篇增设(slTerminal 旧栈现状,随 Tauri 消亡;新 workspace 是否重立归 01 篇骨架)。

### 文档治理签名面

**因果 note 固定结构**(规则照抄 pebrel `architecture/notes/AGENTS.md`,载体改名):

```text
architecture/notes/<owning-path>/<YYYY-MM-DD>-<kebab-case>.md   # 一决定/一事故一文件
必备九段(单篇 ≤200 物理行):Status / Context / Evidence / Decision /
  Rejected alternatives / Consequences / Validation / Supersedes / Revisit when
```

- 路径镜像 owning code path(`architecture/notes/slterm_app/...`、`architecture/notes/packaging/...`);不建全局 INDEX.md,由最近模块 CLAUDE.md 指路。
- 接受后不改写;演进用新 note 标 `Supersedes` 互链,旧 note 只加 `Superseded by` 短指针。
- 何时写:依赖方向、核心 ownership、持久化格式、协议、线程/生命周期、重要性能合同、治理变更、易被遗忘的跨层事故。何时不写:样式、普通单文件修复、机械重构、常规依赖升级。

**adr.md ↔ notes 互链格式**:adr 条目沿革行可写「成因档案见 `architecture/notes/<path>`」;note 的 Context 段可写「当前生效形态见 `.claude/adr.md` ADR-NNNN」。决策被推翻时:adr.md 按既有约定原位收敛 + 沿革;被推翻的成因由 Supersedes 链到的新 note 存档。

**release notes 模板**(照抄 pebrel `docs/release-notes/v1.3.0` 起双语结构):

```markdown
# slTerminal X.Y.Z

## English
### Added / ### Fixed / ### Improved
- 用户可感知变化,描述可核验的用户结果;禁用内部类型名/辅助函数/提交标题。

## 中文
### 新增 / ### 修复 / ### 改进
- 与英文表达同一事实。

---
**SHA256**
- `slTerminal-vX.Y.Z-windows-x64-setup.exe`: `<hash>`
- `slTerminal-vX.Y.Z-windows-x64.zip`: `<hash>`
```

## 数据流与状态机

### 发布链数据流(一次性,无持久状态)

```text
CHANGELOG.md 完成(人工前置)
  → build-windows-product.ps1          # CARGO_TARGET_DIR 隔离,单一 feature 图
  → build-installer.ps1 / package-release.ps1
      ├─ 必填文件存在性检查(红:缺件)
      ├─ Assert-FreshBinaries(红:二进制早于最新源码 mtime)
      ├─ --version 回读比对(红:staged exe ≠ 本次版本)
      ├─ isl pin 校验(红:哈希不符拒用,重新下载再校验)
      ├─ staging → Assert-Manifest 全等(红:多一件少一件)
      └─ ISCC 编译 / zip 原子替换
  → 钉测试两件(归 11 篇:installer.tests.ps1 / package-release.tests.ps1)
  → verify-release.ps1(资产名全等 + 尺寸 + PE 魔数 + SHA256SUMS)
  → 人工核验清单(scripts/release-checklist.md)+ 双点验收 → 结论落 notes
```

每一环都是「读声明/读文件 → 纯判定 → exit code」,失败即中止,无续跑态。`-SkipBuild` / `-AllowStale` 是自测旁路,正式发布路径上被发布纪律禁绝(scripts/CLAUDE.md 登记 + 钉测试反扫)。

### 新鲜度判定状态(双脚本同判据)

`stale ⇔ binary.LastWriteTime < max{mtime | 全部成员 crate 与根 Cargo.toml 的 .rs/.toml}`。基准取源码最新改动而非「本次运行开始时刻」——cargo 对未变更目标不重链接,后者会漏判(pebrel 2026-08-18 事故:修复已写进源码,包里是前一晚的 exe)。slterm 侧源码根 = 01 篇 workspace 成员目录 + 根 Cargo.toml;`..\gpui-component-fork` 这类仓外路径不迁(GPUI 依赖走 git rev,无本地路径依赖)。预编译供给件(NuGet ConPTY 字节经 `include_bytes!` 进 exe)不参与判定。

### 文档双轨生命周期

```text
决策诞生 ─┬─ 当前规则面 → adr.md 新条目(索引表 + 当前有效形态)
          └─ 成因档案面 → architecture/notes/ 新 note(九段)
决策演进 ─┬─ adr.md:原位收敛改写 + 沿革行(禁文末追加块)
          └─ notes:新 note Supersedes 旧 note;旧 note 只加 Superseded by 短指针
决策撤销 ─┬─ adr.md:条目收敛为「已撤销」或删除 + 沿革
          └─ notes:档案永不删;新 note 记录撤销原因与 Revisit when 命中情况
```

补丁/变通的撤销条件 = note 的 `Revisit when` 段,这是「补丁必记撤销条件」的机制化载体;与「未来最优、允许大重构」正交互补——重写越自由,成因与废弃路径的档案越必要。

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单(pebrel → slTerminal,每项一行)

| 来源(pebrel 路径 / 符号) | 落位 | 因果链一句 |
|---|---|---|
| `scripts/installer.iss` 全参数化骨架 | `scripts/installer.iss` | 脚本零硬编码版本 = 版本错打事故的结构性防线;裁剪见关键类型节 |
| `scripts/installer.iss` 的 `PrivilegesRequired=lowest` / `MinVersion=10.0.17763` / 固定 AppId / lzma2/max+SolidCompression / WizardStyle=modern / SetupLogging / UsePreviousAppDir | 同文件 | 用户级安装咬合单实例定位;Win10 下界咬合产品定位;压缩是体积预算门面 |
| `scripts/installer.iss` 双语向导(英文内置 + ChineseSimplified.isl 固定提交 + SHA256 pin,见 `build-installer.ps1` 的 `$translationUrl`/`$translationSha256`) | `scripts/build-installer.ps1` 同位置 | 防发布构建悄悄接受被替换的上游翻译内容 |
| `scripts/build-installer.ps1` 新鲜度与版本核验链(`Assert-FreshBinaries` 判据、`--version` 回读、必填文件检查、ISCC 探测序、`-SkipBuild`/`-AllowStale` 限自测) | 同名 | 「发布包必须来自全新构建」是打包第一纪律;版本回读防「装进去的是旧 exe」 |
| `scripts/build-windows-product.ps1`(CARGO_TARGET_DIR 隔离共享构建) | 同名 | 安装器与 zip 同源同 feature 图;防打死运行中实例 |
| `scripts/package-release.ps1`(`Assert-Manifest` / `Remove-StageSafely` / 临时 zip 原子替换 / zip 二次读回比对) | 同名 | 清单即合同:多一个少一个都红;清理边界防误删;原子替换防半包 |
| `scripts/windows-package-architecture.ps1`(`Assert-WindowsPackageArchitecture` PE machine 头校验) | 同名 | 架构错标(zip 名与内容不符)的机械防线 |
| `scripts/installer-migration.iss` 头部 `#ifdef AcceptanceFixture` 条件编译夹具模式(仅模式,迁移本体不建) | `scripts/installer.iss` AcceptanceFixture 分支 | 安装器自测产出隔离 AppId 的验收夹具,不污染真实注册表卸载项 |
| `scripts/stable_release.py` 的 `expected_asset_names`/`validate_assets`/`_check_magic`/`checksum_text` 与 `preview_release.py` 的 `MIN_ASSET_SIZE`/`verify_binary_freshness`/`validate_version`/`sha256` 纯逻辑 | `scripts/verify-release.ps1`(新建,PS1 重写) | 资产名集合 + 尺寸下限 + PE 魔数 + SHA256 清单三件校验;编排面不采纳 |
| `packaging/AGENTS.md` 发布核验纪律全文(UTF-8、只显式暂存、禁 SkipBuild/AllowStale、允许路径、非强制推送、结束前核验清单、二进制标签不随文档移动) | `scripts/CLAUDE.md` 发布节 + `scripts/release-checklist.md` | 发布事故教训集;无 CI 更依赖本地纪律;「创建命令成功 ≠ 发布核验完成」 |
| `docs/release-notes/AGENTS.md` 纪律(只写有证据的用户可感知变化、双语同事实、同步纪律、Issue 引用前读原文、标明开放范围) | `release-notes/CLAUDE.md` | release notes 防腐纪律;「GitHub Release 正文同步」条改写为「安装器与便携包内置 CHANGELOG 与源文件同步」 |
| `architecture/notes/AGENTS.md` 因果 note 规则全文(九段结构、Supersedes 链、Revisit when、何时写/不写、禁全局 INDEX) | `architecture/notes/CLAUDE.md` | 「补丁/变通必记撤销条件」的机制化载体;与 adr.md 双轨互链 |
| `AGENTS.md` 全局规则「最近路径优先 + 模块细节不回填根规则」两句 | 根 CLAUDE.md 文档规范节 | 防根文件膨胀的元规则 |
| `docs/AGENTS.md` 事实文档防腐条款(只记核验事实、演进带锚、不手抄可自证清单、单篇单一职责) | 根 CLAUDE.md 文档规范节 | 与 ADR-0011 互补:0011 管「什么不写」,此管「写下的怎么不腐」 |
| `docs/architecture.md` 的 ownership map 双列形态(owns / must not become) | 根 CLAUDE.md 架构节聚合表 | 模块边界合同:正向职责 + 负向腐化禁线;各模块 CLAUDE.md 仍是细节真值 |
| 根 `Cargo.toml` `[profile.release]` 体积工程注释纪律(冷代码 s + 热路径钉 O3 + 改动必重跑钉测试) | 根 `Cargo.toml`(结构归 01 篇) | 体积预算是「预算值 + 防退化机制」,钉包清单本篇登记 |
| `THIRD-PARTY-NOTICES`(逐组件归属段结构:组件名/上游与版本/许可证/派生与改编) | `THIRD-PARTY-NOTICES` | GPL-3.0 再分发合规;fork 修改上游 crate 必须声明基线与补丁位置 |
| `licenses/` 组件专属许可证全文随包收录(`installer.iss` [Files] 打入 {app}\licenses) | `licenses/` | 组件许可证要求随分发附全文;内容按 slterm 真实依赖裁剪 |
| `LICENSE`(GPL-3.0 全文) | `LICENSE` | 派生合规义务(01 篇 M0.4 已落,本篇登记四落点) |
| pebrel `scripts/prepare-windows-runtime.ps1` 的「NuGet 固定版本 + 包级/文件级双 SHA256 + PE machine 头」校验链 | 02 篇 Win10 NuGet 捆绑流程的校验补强(参考级) | 部署形态不采纳(include_bytes! 嵌入,ADR-0005);只补校验缺口,缺口落 notes |

### 改名映射引用

全文改名以 01 篇「改名映射单点表」为唯一权威,本篇不另立。本篇直接消费点:P-4(Inno AppName / OutputBaseFilename 品牌位 = `slTerminal`)、P-3(AUMID `com.slterminal.terminal` 写入 [Icons] AppUserModelID)、B-1/B-2(bin `slterm` / `slterm-hook` 进 [Files])、B-4(skills 目录,落位经本篇 assets/ 裁定)、P-6(crate 元数据 homepage/repository = 发布坐标同根,M11 随 workspace 成员 Cargo.toml 补齐)。pebrel 双品牌参数(`PackageBrand=NebulaTerminal|Pebrel`)与 `PEBREL_*`/`NEBULA_*` 双名兼容层整体不迁。

### 缝合点

1. **01 篇 × 合规族**:LICENSE 全文、THIRD-PARTY-NOTICES、licenses/ 的落地归 01 篇 M0.4;本篇定四落点其余三件(Cargo.toml license 字段 / 安装器页 / 关于页)与 notices 内容裁剪纪律。**出入登记**:01 篇根 Cargo.toml 草稿 `[workspace.package]` 缺 `license = "GPL-3.0"` 行——本篇登记为落点二,M0.2 落骨架时补入。
2. **01 篇 × notes 载体名**:01 篇照抄表写 `architecture/notes/AGENTS.md`,spec 分片 12 采纳点 2 裁 `architecture/notes/CLAUDE.md`——本篇按 spec 取 CLAUDE.md(命名单轨:全仓规则文件统一 CLAUDE.md,spec 不采纳点 2)。
3. **01 篇 × ownership map 落点**:01 篇照抄表写 `docs/architecture.md` 重写版,spec 分片 12 采纳点 5 裁收编根 CLAUDE.md 架构节、不建独立文件——本篇按 spec;docs/ 全目录 M11 消亡,独立文件无落点。
4. **04 篇 × skills/schema 资产归位**:04 篇开放问题 1 把 `runtime-api-v1.schema.json` 与 skills 目录的正式归位裁定权交本篇——本篇裁定落根级 `assets/`(见改造节 2);04 篇 `runtime_skills.rs` 的 `include_str!("../../../../docs/skills/slterm-runtime/...")` 路径随资产归位同步改。
5. **09 篇 × 发布坐标与资产合同**:09 篇 `update_check.rs` 的 `parse_latest_release`/`windows_x64_installer_names`/`select_windows_x64_installer`/`checksum_from_release_body` 消费本篇常量与命名模板;坐标真值回填见开放问题 1;`update-test-source` 本地假 release 演练通道以 `SLTERM_RELEASES_*` 可覆盖为前提(覆盖机制归 09)。
6. **11 篇 × 钉测试边界**:`installer.tests.ps1` 钉的对象 = 本篇 iss 合同(参数化值、AppId GUID、双语 isl pin、产物命名);`package-release.tests.ps1` 钉的对象 = 本篇 manifest、新鲜度链、profile 数值与体积预算。两件测试形态归 11,被钉契约数值归本篇。
7. **02 篇 × ConPTY 随包边界**:ConPTY 字节经 `include_bytes!` 内嵌(ADR-0005),manifest 不含 `runtime/conpty.dll` / `runtime/OpenConsole.exe`——「禁双份字节」边界照抄 pebrel 自 1.1.0 起的 zip 做法;THIRD-PARTY-NOTICES 的 ConPTY 归属段(Windows Terminal 项目 + NuGet 版本 pin)保留,版本号与 02 篇捆绑真值同源。
8. **03 篇 × 卸载清理钩**:[UninstallRun] 调 `slterm.exe setup-ai --remove`(子命令面归 01 篇 B-3,安装权威归 03 篇);03 篇须保证该子命令在无窗口、静默(skipifdoesntexist/runhidden)场景可用。
9. **06 篇 × 关于页**:四落点之四 = slterm_app 关于面板(GPL-3.0 声明 + THIRD-PARTY-NOTICES 入口),品牌值消费 01 篇 `brand::NAME`;面板形态归 06 篇设置壳,本篇只锚合规义务。
10. **manifest 单源**:`package-release.ps1` 的 `$manifest` 与 `build-installer.ps1` 的 `$requiredFiles` 表达同一文件集(口径:zip 收录全集,iss [Files] 另含安装器专属布局)——两处派生自同一期望集,钉测试全等比对防漂移(11 篇)。

## 改造 / 移植 / 新建设计

### 1. 文档双轨终态落位

**文件面清单**(新世界全部文档面;除此之外不建):

```text
CLAUDE.md                       # 根:全局规则 + 架构节(ownership 双列)+ 文档规范节
slterm_app/CLAUDE.md 等 8 crate # 模块细节真值(随各 crate 迁入建立)
scripts/CLAUDE.md               # 脚本族规则 + 发布纪律节
architecture/notes/CLAUDE.md    # 因果 note 规则(九段/Supersedes/Revisit when)
architecture/notes/**           # 成因档案
.claude/adr.md                  # 当前生效决策(保留,维护约定不动)
CONTEXT.md                      # 纯术语表(保留)
.claude/test-exemptions.md      # 豁免登记(保留,重审归 11 篇)
CHANGELOG.md                    # 双语,随包内置
release-notes/CLAUDE.md         # release notes 纪律
release-notes/unreleased.md + vX.Y.Z.md
scripts/release-checklist.md    # 发布人工核验清单(随脚本走)
README.md + README.zh-CN.md     # M11 双语重写
```

**根 CLAUDE.md 文档规范节新增条款**(M11 重写时并入,与既有快照数字禁令/行号引用禁令/代码自证原则并列):

- 渐进披露元规则两句:「规则按目录分层,遵守目标文件路径上最近的 CLAUDE.md」「模块细节留在模块目录,不回填根文件」。
- 事实文档防腐四条:只记经核验的当前事实,计划/待办/临时记录不入文档;表达演进必须带版本/日期/状态锚,禁无锚点「以前、现在、不再」;不手抄可从 schema/类型定义/命令帮助/生成文件直接得到的清单,链接单一权威来源;单篇单一职责、可审查长度。
- adr/notes 分工与互链规则(关键类型节已给格式)。

**adr.md 与 notes 分工模型**(spec 分片 12 采纳点 3 落地):adr.md 收「当前生效决策的当前形态」——产品定位、硬约束级,数量少、需快速索引;notes 收「决策的完整成因档案与被推翻决策的存档」——含被否决方案、证据、撤销条件、事故因果,随时间增长、按代码路径分散。两者是两种相反的正确(生效态要原位收敛、档案态不可改写),不可合并。同一主题双登记时条目内互链。M11 沉淀动作:M0-M10 各篇「待沉淀决策」落定后,当前规则面进 adr.md,成因档案面进 notes。

**根 CLAUDE.md 的 Tauri 时代 → GPUI 时代改写范围**(M11.3 执行表):

| Tauri 时代条目 | GPUI 时代去向 |
|---|---|
| 两进程模型 / 架构节 | 单进程 GPUI 模块化单体 + workspace 结构 + ownership 双列 |
| 硬约束 1(前端不碰 OS)/ 3(命令注册)/ 4(DTO 单源 CP-024)/ 5(面板封闭)/ 7(布局单点)/ 8(会话元数据单点)/ 12(store 纯状态) | 消亡或以 Rust 形态重建(重建形态归 01/04/05 篇,根文件只引结果) |
| 硬约束 2(模块不穿透)/ 6(配色单点)/ 9(平台分支收敛)/ 10(权限最小化)/ 11(测试覆盖)/ 13(注册表契约)/ 14(SEC-18) | 保留改写:载体从 Tauri 模块/workspace 换成新 crate 面;SEC-18 不变 |
| 命令节(npm/tauri 族) | cargo 族 + scripts/ 打包链 + 门禁三件套 + git hooks 启用指令 |
| 测试策略节(四级金字塔 L1-L4) | 新四层(11 篇目标形态表)+ TQ-COV-06 预防性知识 |
| package.ps1 一键打包行 | scripts/ 四件 + verify-release + checklist |
| pebrel 重构基线节 | 保留(baseline commit 与增量操作法是历史锚点) |

### 2. 永久资产正式归位 `assets/`(裁定 04 篇开放问题 1)

裁定:docs/ 临时稿目录 M11 整体消亡,一切永久随包资产落仓库根 `assets/`:

```text
assets/skills/slterm-runtime/SKILL.md            # 编译期 include_str! 内嵌 + 随包分发
assets/skills/slterm-runtime/agents/openai.yaml
assets/runtime-api-v1.schema.json                # 协议 schema 单文件,随包 + describe.schema 可达
assets/runtime-control-api.md                    # 协议事实文档真值(随包即文档面,不建 docs/ 大杂院)
```

连锁改动:04 篇 `runtime_skills.rs` 的 `include_str!` 相对路径改指 `assets/`;iss [Files] 与 zip manifest 的 Source 路径同指(关键类型节已按此写)。仓内协议文档单一真值源 = 随包这份——「链接单一权威来源」防腐条款的直接应用。

### 3. Inno 打包链落地细节

**manifest(slterm 版 `$manifest` 期望集,zip 与 iss 共同真值)**:

```text
slterm.exe                                       # 主程序(ConPTY/着色器/字体字节均内嵌)
runtime/slterm-hook.exe                          # hook 小进程(独立 bin,必须随包)
README.md / README.zh-CN.md
docs/CHANGELOG.md                                # 随包内置 → 发布前必须完成 Changelog
docs/runtime-control-api.md / docs/runtime-api-v1.schema.json
skills/slterm-runtime/SKILL.md / skills/slterm-runtime/agents/openai.yaml
licenses/LICENSE / licenses/THIRD-PARTY-NOTICES / licenses/LICENSE-LATIN-MODERN-MATH
```

(左列 = 包内相对路径;`licenses/LICENSE-LATIN-MODERN-MATH` 随 08 篇数学字体内嵌义务存续,若 08 篇砍字体则连同 notices 对应段一并销。)

**新鲜度链补强点**:pebrel 源码根含 `..\gpui-component-fork\crates` 本地路径依赖,slterm 无(GPUI 走 git rev 钉版)——slterm 版 `Assert-FreshBinaries` 源码根 = 8 个成员 crate + 根 Cargo.toml;git rev 依赖的源码变更由 `--locked` + Cargo 自身重编译保证,不进 mtime 判据。

**发布核验清单文本化**(`scripts/release-checklist.md`,照 pebrel `packaging/AGENTS.md` 末条核验面):标题/标签(若开 GitHub Release)/产物文件名/大小/SHA256 与 SHA256SUMS 一致/版本回读/setup.exe 在干净 Win10 或另一台 PC 安装-启动-卸载全流程/zip 解压直跑/卸载后无残留(安装目录、卸载注册表项;AI CLI hook 注入已清)。「命令成功 ≠ 核验完成」为清单首行。

### 4. 体积工程落地与校准纪律

落地三步:M0.2 落 profile 结构(01 篇)→ M3 后首个可运行 release 构建实测 setup.exe 体积 → 以「实测值 + 裕度」钉入 `package-release.tests.ps1`(11 篇承载),注释标明校准日期与实测值。初始参考 30MB(pebrel 同预算,仅作上限起点不作合同)。改动 `[profile.release]` 或钉包清单必须重跑钉测试——profile 注释内钉死此纪律(pebrel 同形态)。防退化语义:钉的是「预算值 + 机制」,超预算即红;预算收紧(降值)永远合法,放宽(升值)须在提交信息说明原因。

### 5. LICENSE 四落点与合规族内容裁剪

| 落点 | 内容 | 落地阶段 |
|---|---|---|
| `LICENSE` 全文 | GPL-3.0,版权行 LiuShenLan | M0.4(01 篇) |
| 根 `Cargo.toml` `[workspace.package] license = "GPL-3.0"` | spdx 标识;成员 crate 经 `license.workspace = true` 继承 | M0.2(缝合点 1 登记补入) |
| 安装器许可页 | `LicenseFile={#RepoRoot}\LICENSE` | M11.1 |
| 关于页 | slterm_app 关于面板:GPL-3.0 声明 + notices 入口 | M11.3(面板形态归 06 篇) |

spec 分片 12 采纳点 17 的 `package.json` 落点随 npm 族 M0 全删而消亡,四落点以本篇表为准。逐文件 GPL 头注释不采纳(双方均无头注释传统;归属通知义务由 LICENSE 全文 + Cargo.toml license 字段 + notices 承载)。git 历史 MIT 时期文件保留历史原貌。

**THIRD-PARTY-NOTICES slterm 版裁剪**(形态照抄,内容按 fork 后真实依赖重写):

- 删除:Lua/mlua 段(Lua 栈砍)、SSH bashrc 段(SSH 砍)、ARM64 NuGet 相关表述(arm64 不建)。
- 保留改写:ConPTY 段(Windows Terminal 项目 + NuGet 版本 pin,真值归 02 篇)、GPUI 段(gpui/gpui-component Apache-2.0;fork 基线与补丁记录位置改指「根 Cargo.toml [workspace.dependencies] 注释块」——slterm 不建 [patch.crates-io],pebrel 原文位置声明失效)、字体段(Latin Modern Math,随 08 篇存续)、agent_detection 的 Herdr 归属段(03 篇屏幕证据迁入则保留)。
- 头部声明:本项目许可证与第三方组件各自许可证互不影响。
- Tabby shell 检测段去留 = 开放问题 2。

### 6. release notes 双语形态与同步纪律

- 面:根 `CHANGELOG.md`(双语,随包内置)+ `release-notes/unreleased.md`(进行中,照 pebrel 指针形态)+ `release-notes/vX.Y.Z.md`(按版本)。
- 纪律(照抄 pebrel `docs/release-notes/AGENTS.md`,两处改写):「CHANGELOG/版本 notes/GitHub Release 正文三处同步」改写为「源文件与安装器、便携包内置的 CHANGELOG 同步——正式构建前完成 Changelog,因包内置 CHANGELOG」;Issue 引用条款在 slTerminal 无公开 issue  tracker 期间登记为「有 tracker 后生效」,引用 URL 模板随发布坐标定。
- 文案纪律不变:只写已实现且有代码/构建/真实运行证据的用户可感知变化;探针与内部重构不写成已交付功能;开放或部分覆盖的问题准确标明范围。
- 版本 notes 末尾 SHA256 块由 verify-release.ps1 输出的 SHA256SUMS 填入,禁手抄。

### 7. package.ps1 旧链路消亡映射

`.claude/package.ps1`(tauri build → 单 exe 裸打 zip)整体消亡:M0 随 npm/Tauri 栈删除,git 历史保留。能力映射:构建 → `build-windows-product.ps1`;zip → `package-release.ps1`(补回旧链路无清单、无合规文件、无新鲜度检查三大缺口);版本参数 → 两脚本 `-Version`(缺省回读 Cargo.toml,旧链路硬编码默认 0.2.0 的形态不迁)。「一键打包」习惯由 M11 的单一发布入口承接:`scripts/release.ps1`(薄编排:调四件 + 钉测试 + verify-release,逐失败即停)——该入口是新建件,非 package.ps1 改造。

## 测试点清单

| 测试 | 层级/形态 | 机制 |
|---|---|---|
| `installer.tests.ps1`(钉参数化值回读、AppId GUID、isl pin、产物命名、裁剪项不存在) | PS1 自断言 | **归 11 篇随迁,本篇只给被钉合同**;隔离设计照 pebrel(空输出目录 + 输出路径拒逃逸 target/) |
| `package-release.tests.ps1`(钉 profile 数值 debug=0/strip、共享构建入口、Assert-FreshBinaries 存在、产物集全等、体积预算值) | PS1 自断言 | **归 11 篇随迁**;预算值首版校准前以「登记值 + 注释标明首版校准」入仓 |
| `verify-release.ps1` 自测:资产名全等正反例、空资产拒收、非 PE 头拒收、SHA256SUMS 格式 | PS1 自断言(归 scripts/tests/ 族) | 新建,对夹具目录执行;防「校验脚本自身静默失效」 |
| 发布坐标一致性:`SLTERM_RELEASES_*` 五常量单点定义、09 篇消费处无第二份字面量 | 脚本断言(可并入 governance 件) | 新建;grep 断言字面量 URL 仅出现于 releases.rs |
| manifest 含合规文件:`licenses/LICENSE`、`licenses/THIRD-PARTY-NOTICES` 在 `$manifest` 与 `$requiredFiles` 双处 | 钉测试内断言(11 篇承载) | GPL 再分发合规的机械防线 |
| notes 九段结构与链接不悬空 | python unittest(01/11 篇 governance 件 `NOTE_REQUIRED_SECTIONS` 同族) | 照抄;adr.md 沿革行中的 notes 链接一并纳入悬空检查 |
| CHANGELOG 同步:构建前 `CHANGELOG.md` 含本次版本号节 | `build-installer.ps1` 前置断言 | 「包内置 CHANGELOG」的配套防线,防旧说明进新包 |
| 新鲜度链反例:改源码不重构建 → 两打包脚本必须红 | 钉测试用例(11 篇承载) | 防复发锚 = pebrel 2026-08-18 事故 |
| 文档面零越位:仓内(docs/pebrel-* 外)无 `AGENTS.md` 文件;无 docs/architecture.md 等被裁文件 | 脚本断言(governance 件扩) | 命名单轨与「不建 docs 大杂院」裁定的机械防线 |

## 阶段归属与出口标准

对照 00-roadmap M11 细化为四子步;M0 两件前置。

**M0.2 附带(归 01 篇骨架步,本篇登记)**:根 Cargo.toml `[workspace.package]` 补 `license = "GPL-3.0"`。出口 = governance 断言(01 篇合规断言扩一项)。

**M0.4 附带(归 01 篇合规步,本篇登记)**:`architecture/notes/CLAUDE.md` 规则文件随门禁三件套同批建立(五处架构契约文件同批:file-budgets / dependencies.toml / 禁名清单 / notes 规则 / 打包脚本骨架)。出口 = governance 件的 note 九段检查可跑。

**M11.1 打包链落地**:scripts/ 五件(build-windows-product / build-installer / package-release / installer.iss / verify-release)+ release.ps1 入口 + scripts/CLAUDE.md 发布节 + release-checklist.md。出口(全机验):
- `powershell -File scripts/release.ps1 -Version <X.Y.Z>` 全链跑出 setup.exe + zip + SHA256SUMS;
- 新鲜度反例红(改源码不重构建,两脚本各红一次);
- `--version` 回读比对过;isl pin 校验过;
- installer.tests.ps1 / package-release.tests.ps1 / verify-release 自测全绿(11 篇出口联动)。

**M11.2 体积工程校准**:首版 release 构建实测 setup.exe 体积 → 预算值钉入钉测试。出口 = 预算钉测试绿;实测值、校准日期、裕度三者入注释。

**M11.3 文档重写与沉淀**:根 CLAUDE.md 族按改造节 1 表重写;crate CLAUDE.md 补齐;M0-M10 各篇待沉淀决策落 adr.md/notes;CONTEXT.md 补新术语;README 双语重写;assets/ 归位连锁改完(04 篇 include_str 路径);docs/pebrel-refactor/ 与 docs/pebrel-design/ 删除。出口(全机验):
- 仓内(docs/ 已删后)grep 无 `pebrel-refactor`/`pebrel-design` 引用残留;
- 文档面零越位断言过;governance 件(notes 九段 + 链接不悬空)绿;
- 禁名门禁全树模式过(01 篇 D4 默认口径:临时稿删除后切全树常挂)。

**M11.4 首发核验**:verify-release.ps1 全绿;release-checklist.md 人工全勾(含干净环境安装-启动-卸载、zip 直跑、卸载无残留、另一台 PC 验收);验收结论落 `architecture/notes/packaging/`。出口 = checklist 全勾 + 结论 note 入库。

## 待沉淀决策

> [待沉淀] **D12-1 · Inno AppId GUID 新立钉死**。真实权衡:新立 GUID = 与 Tauri 时代 identifier(`com.liush.slterminal`)彻底切割,无历史关联;续用概念不存在——slTerminal 从未有 Inno 安装基(旧链路只产 zip),无 GUID 可续。难逆点:GUID 随首发公布即成为续装/覆盖升级/卸载标识,更改 = 用户侧双卸载项并存。意外因素:09 篇自动更新的「同安装基」判定也锚 AppId,与更新链耦合。截止:M11.1 installer.iss 落盘前定(默认:新生成一个 v4 GUID 钉入,夹具 GUID 另立一个,两值永不变更)。

> [待沉淀] **D12-2 · 体积预算值首版实测校准**。真实权衡:30MB 初始值照 pebrel 同预算,但 slterm 依赖面不同(无 Lua/无 SSH 栈/无捆绑字体 vs 27 家 hook 面),实测可能系统性偏离;定严 = 常态误红,定松 = 退化漏报。难逆点:数值入仓即契约,放宽须说明原因,收紧自由——首发值定高即永久失去收紧空间的可信基线。意外因素:M3 首建与 M11 全量功能间体积增长不可预知,过早钉死可能被迫连升。截止:M11.2 全量功能态实测后定(默认:实测值 + 15% 裕度,注释登记实测值与校准日期)。

> [待沉淀] **D12-3 · 永久资产正式归位 `assets/`**。真实权衡:落根级 `assets/` = 与 docs/ 临时稿彻底切割、随包路径稳定,代价是仓库根目录多一层;落 `slterm_app/assets/` = 资产贴近消费 crate,代价是 include_str! 跨 crate 相对路径更深、且 skills/schema 的消费者(app 内嵌 + 安装器 + zip)本就跨 crate。难逆点:资产路径随包分发后成为外部可依赖面(09 篇更新链、AI CLI skill 安装路径),迁移即兼容负担。意外因素:04 篇开放问题 1 已把裁定权交本篇,而 docs/ 消亡前提使 spec 分片 12 原文的 `docs/release-notes/`、`docs/skills/` 落点全部失效。截止:M11.1 manifest 落盘前定(默认:按本篇改造节 2 执行;release-notes 同理落根级 `release-notes/`)。

> [待沉淀] **D12-4 · 首发版本线**。真实权衡:续 0.3.0 = 与 slTerminal 0.2.0 历史用户感知连续,但 09 篇更新链的版本比较会把 0.2.0(Tauri 时代)误认为可更新对象;重计 1.0.0/0.1.0 = 语义割裂清晰,代价是版本号回退观感。难逆点:首发后版本线不可回改;更新链「版本大于当前才提示」语义会把跨架构旧版卷进来。意外因素:0.2.0 用户是真实存在的两台 PC 部署(用户既有习惯)。截止:M11.4 首发前定(默认:续 0.3.0 并在更新链对 <0.3.0 一律视为「需手动重装」——Tauri 时代安装基不构成 Inno 可更新对象)。

## 开放问题

1. **`SLTERM_RELEASES_OWNER` / `_REPO` 真值**:GitHub 仓是否建立、owner 是否 `LiuShenLan`、仓名 `slTerminal` 是否可用,均未定。默认按关键类型节常量占位;若首发不开 GitHub Release,09 篇更新链以 `update-test-source` 本地通道验收,公网坐标留占位回填。
2. **THIRD-PARTY-NOTICES 的 Tabby 归属段去留**:pebrel 的 Windows 安装 shell 检测(注册表查找序 + 品牌图标)derive 自 Tabby;slterm 侧 shell 探测 = 02 篇并入的自有白名单深检与 pebrel 对应物的最终合成面未定——若并入 pebrel 实现则声明保留,若纯自有实现则删。截止:M2 slterm_terminal 落定后,由 02 篇作者回填本篇合规裁剪清单。
3. **arm64 可选项登记处**:iss 的 arm64 参数化形态照抄保留但构建不做(spec 已定);「Windows on ARM 后续可选项」的撤销条件是否立 note(`architecture/notes/packaging/`)登记?默认立一篇短 note(Revisit when = 出现 arm64 设备需求或 GPUI 上游 arm64 支持变化),随 M11.1 同批。