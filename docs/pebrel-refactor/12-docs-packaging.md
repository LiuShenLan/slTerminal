# pebrel 重构优化 spec · 分片 12：文档纪律与打包发布

baseline commit：`e537d528c508e8607d0f5f9fd25e5902f40d661e`（pebrel，Rust + GPUI 模块化单体）

## 优化面

1. **文档治理三层裁定**——既定全局决策「文档纪律混合」落地：CLAUDE.md 渐进披露 / adr.md / CONTEXT.md / test-exemptions.md 四件套保留，补 pebrel 的 architecture/notes 因果 note 机制（补丁/变通必记撤销条件）与 docs 事实文档防腐条款；file-budgets 门禁细节归分片 11。逐条裁定 AGENTS.md 分层规则、notes 固定结构与 Supersedes 链、docs/ 侧三份治理文档（project-constraints.md / architecture.md ownership map / engineering-evidence.md）的收编或拒收。
2. **打包发布**——既定全局决策「Inno 安装器为主形态」落地：installer.iss + 构建脚本族 + 体积工程（opt-level="s" + 热路径钉 O3 + 预算钉测试）+ 发布核验纪律 + THIRD-PARTY-NOTICES/licenses 合规族 + release notes 形态；zip 便携形态去留给出裁定；ConPTY 随包部署与分片 02 已裁形态对齐。
3. **LICENSE 切换**——MIT→GPL-3.0 的四个落点（LICENSE 文件 / Cargo.toml license / package.json / notices）与「不逐文件头注释」边界。pebrel 本体即 GPL-3.0，fork 并入其代码后切换是派生许可合规要求而非风格选择。
4. **slTerminal 侧现状对照**——`.claude/package.ps1`（改造为 Inno 主链路）、`.claude/adr.md`、`CONTEXT.md`、各模块 CLAUDE.md、test-exemptions.md 逐面裁定。
5. **与分片 11 的协调改判**——分片 11 不采纳点曾裁定「发布相关测试不随 fork 走，打包走 package.ps1 既有链路」；按 Q17 全局决策（Inno 为主）改判：预算钉测试与安装器自测随迁，分片 11 该条随本 spec 定稿同步修订。

## 采纳点

### 文档治理

1. **照抄延续 + 补条款：AGENTS.md 分层规则核心两句**。slTerminal 现状：根 `.claude/CLAUDE.md` 的「渐进式披露」「收录判定」「子文件维护」条款（载体 CLAUDE.md 为 harness 原生逐目录自动加载）。pebrel 侧位置：`AGENTS.md` 全局规则「规则按目录分层：先遵守本文件，再读取目标文件路径上最近的 AGENTS.md。模块细节留在模块目录，不回填到根规则」。裁定：载体与收录判定条款不动，将「最近路径优先」「模块细节不回填根规则」两句显式并入根 CLAUDE.md——这是防根文件膨胀的元规则；pebrel 根文件尾「分层入口」清单与渐进披露同构，不另建。

2. **照抄：architecture/notes/ 因果 note 体系全要素**。pebrel 侧位置：`architecture/notes/AGENTS.md`（规则全文）+ 实例 `architecture/notes/packaging/2026-09-21-windows-arm64-portable.md`。要素：路径镜像 owning code path（`<capability>/<YYYY-MM-DD-kebab-case>.md`，一决定/一事故一文件）；禁全局 INDEX.md，由最近模块 CLAUDE.md 指路；note 接受后不改写，演进用新 note 标 `Supersedes` 互链、旧 note 只加 `Superseded by` 短指针；固定结构 Status / Context / Evidence / Decision / Rejected alternatives / Consequences / Validation / Supersedes / Revisit when，单篇 ≤200 物理行；何时写（依赖方向、核心 ownership、持久化格式、协议、线程/生命周期、重要性能合同、治理变更、易被遗忘的跨层事故）与何时不写（样式、普通单文件修复、机械重构、常规依赖升级）。理由：「补丁/变通必记撤销条件」的机制化载体 = `Revisit when` 段；与「未来最优、允许大重构」取向正交互补——重写越自由，成因与废弃路径的档案越必要。落点：slTerminal 版 = `architecture/notes/`（与分片 11 的 file-budgets 同根），规则文件为 `architecture/notes/CLAUDE.md`，镜像路径按 fork 后 crate 名（slterm_*）。

3. **照抄：adr.md 与 notes 并存分工模型**。pebrel 侧位置：`docs/architecture-decisions.md` 头部治理迁移声明（治理迁移后新决策落 notes/，旧记录保持不动作历史档案）。裁定：`.claude/adr.md` 及维护约定（索引表 + 正文恒为当前有效形态 + 条末沿革行）原样保留——adr.md 收「当前生效的架构决策的当前形态」（产品定位、硬约束级，数量少、需快速索引）；notes 收「决策的完整成因档案与被推翻决策的存档」（含被否决方案、证据、撤销条件、事故因果，随时间增长、按代码路径分散）。两者是两种相反的正确（生效态要原位收敛、档案态不可改写），不可合并；同一主题双登记时：adr.md 条目 = 面向未来的当前规则，note = 当初为何选它的档案，条目内互链。决策被推翻时 adr.md 按既有约定收敛改写 + 沿革，旧形态与成因由 Supersedes 链接到的新 note 存档。

4. **照抄：docs 事实文档防腐条款**。pebrel 侧位置：`docs/AGENTS.md` 全部条款。要素：文档只记经过核验的当前事实（系统行为、架构地图、协议、操作手册、验收合同、事故复盘的事实部分）；计划、待办、临时工作记录不入文档；事实变化更新正文，表达演进须带版本/日期/状态锚，禁无锚点「以前、现在、不再」；不手抄可从 schema / 类型定义 / 命令帮助 / 生成文件直接得到的 API 清单，链接单一权威来源；单篇单一职责、可审查长度。理由：与 ADR-0011 代码自证原则互补——0011 管「什么不写」，这些条款管「写下来的怎么不腐」；落点 = 根 CLAUDE.md 文档规范节。

5. **参考：ownership map 双列形态（owns / must not become）**。pebrel 侧位置：`docs/architecture.md` 的「Current ownership map」表。裁定：双列（职责正向合同 + 腐化方向负向禁线）照抄为模块边界合同形态；落点不建独立 `docs/architecture.md`，在 fork 后 workspace 落位时收编进根 CLAUDE.md 架构节（与「目录结构原则」并列、单点维护）——各模块 CLAUDE.md 仍是模块细节真值，根文件只持聚合双列表 + 指向各模块文档。

### 打包发布

6. **照抄：Inno 安装器为主形态及 installer.iss 骨架**。pebrel 侧位置：`scripts/installer.iss`。照抄要素：`#ifndef` 宏默认值 + ISCC `/D` 注入的全参数化（AppVersion / NumericVersion / Configuration / Architecture / PackageBrand，脚本零硬编码版本）；`PrivilegesRequired=lowest` 用户级安装（写 HKCU、无需管理员，与本机单实例产品定位一致）；`MinVersion=10.0.17763`（Win10 下界与产品定位咬合）；固定 AppId GUID（续装/覆盖升级/卸载标识稳定）；`Compression=lzma2/max` + `SolidCompression=yes`（体积预算门面手段）；`WizardStyle=modern` + `SetupLogging=yes`；`UsePreviousAppDir=yes`；产物命名 `{Brand}-v{version}-windows-x64-setup.exe`；双语向导（英文内置 + `ChineseSimplified.isl`，按 build-installer.ps1 的固定上游提交 + SHA256 pin 获取，防发布构建悄悄接受被替换的上游内容）。裁剪：arm64 参数化形态照抄但 arm64 构建不做（定位无 arm64 需求，Windows on ARM 登记为后续可选项）；附加集成任务逐项裁决见不采纳点。

7. **照抄：build-installer.ps1 的新鲜度与版本核验链 + 共享构建入口**。pebrel 侧位置：`scripts/build-installer.ps1`（`-SkipBuild` / `-AllowStale` 仅限脚本自测、正式发布一律全新构建；源码 mtime vs 二进制 mtime 陈旧检查；`--version` 输出回读与安装器版本号比对；必填收录文件存在性检查；ISCC 探测序 `ISCC_PATH` → `%LOCALAPPDATA%\Programs\Inno Setup 6` → ProgramFiles 候选）与 `scripts/build-windows-product.ps1`（`CARGO_TARGET_DIR` 隔离的共享显式构建入口，安装器与 zip 同源同 feature 图）。理由：「发布包必须来自全新构建」是打包第一纪律，现有 `.claude/package.ps1` 无任何新鲜度检查，照抄补齐；版本回读防「装进去的是旧 exe」。

8. **参考：installer-migration.iss 的 AcceptanceFixture 条件编译夹具模式**。pebrel 侧位置：`scripts/installer-migration.iss` 头部 `#ifdef AcceptanceFixture` 隔离 AppId / 注册表键 + `scripts/tests/installer-migration.tests.ps1` 及其 fixtures（同族 `scripts/tests/installer.tests.ps1` 一并参考）。裁定：迁移功能本体不采纳（见不采纳点），`#ifdef` 夹具模式参考采纳——slTerminal 版安装器自测以同一条件编译产出隔离 AppId 的验收夹具，自测不污染真实注册表卸载项；installer-migration.iss 不建。

9. **照抄：package-release.ps1 的 staging 清单全等校验与安全清理边界；zip 便携形态裁定保留**。pebrel 侧位置：`scripts/package-release.ps1` 的 `Assert-Manifest`（staging 实际文件集与期望清单 `Compare-Object` 全等，多一个少一个都红）与 `Remove-StageSafely`（解析后路径必须以输出根 `.stage-` 为前缀否则拒删）与临时 zip 原子替换。zip 便携形态裁定（Q17 去留建议）：**保留为次要形态**——用户既有免安装分发习惯（另一台 PC 直接使用）、与安装器共享同一 manifest 真值源、零额外维护面；现有 package.ps1「单 exe 裸打 zip」无清单、无合规文件、无新鲜度检查，被本体系整体取代。内嵌资源（字体 / ConPTY 字节）不落盘进 zip 的「禁双份字节」边界同 pebrel 自 1.1.0 起的做法。

10. **照抄：体积工程三件套**。pebrel 侧位置：根 `Cargo.toml` `[profile.release]`（`lto = "thin"` / `debug = 0` / `strip = "symbols"` / `codegen-units = 1` / `opt-level = "s"`，注释钉明「冷代码按体积编译、热路径逐个钉回 O3、改动必须重跑预算测试」）与 `[profile.release.package]`（resvg / rustybuzz / taffy / ttf-parser / gpui / gpui-component / nebula_terminal / image 逐个钉 `opt-level = 3`，主 crate 并行 codegen 同 O3）与 `scripts/tests/package-release.tests.ps1`（钉测试：解析 Cargo.toml 断言 `debug = 0` / `strip = "symbols"`、打包必须走共享显式构建、必须含新鲜度守卫、产物集全等比对）。slTerminal 现状：`src-tauri/Cargo.toml` 已有 thin LTO / strip / panic=abort，无体积 opt 策略、无预算钉测试。裁定：机制三件套全抄；热路径 crate 清单按 fork 后实际 crate 面重登记（渲染 / 塑形 / 布局 / VT 对应物）；`panic = "abort"` 现状保留不动。预算数字：30MB 参考作初始值，首个 release 构建实测校准后钉进测试——钉的是「预算值 + 防退化机制」，不照抄 pebrel 数字本身。

11. **照抄：发布核验纪律**。pebrel 侧位置：`packaging/AGENTS.md`（发布前完整阅读发布规则与 release-notes 规则；发布命令 UTF-8；只显式暂存本次文件、不清理用户未跟踪探针；正式发布禁 `-SkipBuild` / `-AllowStale`；构建与输出须在用户允许路径；普通非强制推送；结束前核验标题 / 标签 / 正文 / 真实资产文件名 / 空资产标签 / 大小 / SHA256 / 分支与版本标签指向——「创建命令成功不等于发布核验完成」；二进制版本标签不随说明文档修订移动）。理由：发布事故教训集；slTerminal 无 CI 更依赖本地纪律。落点：打包目录 CLAUDE.md（收录判定：每次发版必需），核验清单文本化随脚本走。

12. **参考：stable_release.py / preview_release.py 的资产校验纯逻辑**。pebrel 侧位置：`scripts/stable_release.py` 的 `expected_asset_names` / `validate_assets`（PE 魔数 + `MIN_ASSET_SIZE` 防空资产）/ `sha256` / `checksum_text`；`scripts/preview_release.py` 的 `verify_binary_freshness` / `validate_version`。裁定：GitHub Release 编排面不采纳（见不采纳点）；资产名集合 + 尺寸下限 + SHA256 清单三件校验收敛为本地发布核验脚本，对 dist/ 产物执行，SHA256 清单进 release notes。多平台资产名表收敛为 windows-x64 安装器 + 便携 zip 单面。

13. **参考（边界对齐分片 02）：ConPTY 随包部署形态**。pebrel 侧位置：`nebula_app/build.rs` 的 `deploy_conpty`（构建期拷贝 conpty.dll / OpenConsole.exe 到产物目录、失败仅 warning 不 fail、运行时回退 in-box ConPTY）与 `scripts/prepare-windows-runtime.ps1`（NuGet 固定版本 + 包级/文件级双 SHA256 + PE machine 头校验）。裁定：部署形态不采纳——分片 02 已裁保留 slTerminal 的 `include_bytes!` 嵌入 + `%LOCALAPPDATA%` 提取形态（ADR-0005），不改随包外部文件；runtime 获取的「固定版本 + 双 SHA256 + PE 头」校验链参考采纳为 Win10 NuGet 捆绑流程的校验补强（现有流程已等价则只补缺口，缺口与判定落 notes）。

14. **照抄：THIRD-PARTY-NOTICES + licenses/ 合规文件族**。pebrel 侧位置：`THIRD-PARTY-NOTICES`（逐组件归属段：组件名 / 上游与版本 / 许可证 / 派生或改编了什么，含 conpty.dll 归属 Windows Terminal 项目 + NuGet 版本 pin、修改 fork 的基线与补丁记录位置声明）+ `licenses/`（组件专属许可证全文随包收录，`installer.iss` `[Files]` 打进入安装目录 licenses/）。理由：GPL-3.0 再分发合规要求高于 MIT，且 fork 修改上游 crate（gpui / gpui-component 同族）必须声明基线与补丁；slTerminal 版内容按 fork 后真实依赖与改编重写、形态照抄；合规文件进打包 manifest 必含项（与采纳点 9 咬合）。

15. **照抄：release notes 形态与纪律**。pebrel 侧位置：`docs/release-notes/AGENTS.md` 纪律（只写已实现且有代码 / 构建 / 真实运行证据的用户可感知变化，探针与内部重构不得写成已交付功能；双语结构 Added / Fixed / Improved × 中英 + Contributors + 最终资产 SHA256；CHANGELOG.md 与版本 notes 与发布正文保持同步、正式构建前完成 Changelog——因包内置 CHANGELOG；Issue 引用前读原文挂最准条目；文案描述可核验用户结果，不用内部类型名 / 辅助函数 / 提交标题；开放或部分覆盖的问题准确标明范围）+ `docs/release-notes/unreleased.md` 进行中形态。slTerminal 现状：无 CHANGELOG、无 release notes。裁定：建立根 `CHANGELOG.md`（双语）+ `docs/release-notes/`（进行中 + 按版本）；「多处同步」条款改写为「安装器与便携包内置的 CHANGELOG / notes 与源文件保持同步」（无 GitHub Release 载体）。

16. **照抄延续：slTerminal 文档体系四件保留**。`.claude/adr.md`（原位收敛 + 沿革行 + 索引表）、`CONTEXT.md`（纯术语表，职责不变）、`.claude/test-exemptions.md`（豁免登记单表真值源 + 模块明细两级形态）、各模块 CLAUDE.md（渐进披露载体，fork 后按新 workspace 重建）——原样保留，与采纳点 2 的 notes 并存（分工见采纳点 3）。

### LICENSE 切换

17. **照抄：GPL-3.0 本体与四落点；不逐文件头注释**。pebrel 侧位置：`LICENSE`（pebrel 本体即 GPL-3.0 全文）+ `scripts/installer.iss` 的 `LicenseFile={#RepoRoot}\LICENSE`（安装向导许可页随包）。落点四件：仓库根 `LICENSE` 换 GPL-3.0 全文（版权行 LiuShenLan 保留）；workspace 根 Cargo.toml `license = "GPL-3.0"`（spdx 标识，fork 后多 crate 时各成员同步）；`package.json` license 字段同步；`THIRD-PARTY-NOTICES` 头部声明本项目许可证与第三方组件各自许可证互不影响。理由：pebrel 上游即 GPL-3.0，fork 并入其代码后派生部分必须以 GPL-3.0 兼容方式发布——本切换是派生许可的合规要求而非风格选择；gpui / gpui-component（Apache-2.0）并入 GPL-3.0 项目方向合规（Apache-2.0 → GPL-3.0 单向兼容）；既有代码无第三方贡献者，MIT → GPL 重授权无外部同意障碍。源文件逐文件 GPL 头注释不采纳（pebrel 与 slTerminal 均无头注释传统，GPL 归属通知义务由 LICENSE 全文 + Cargo.toml license 字段 + notices 承载）。

## 不采纳点

1. **AGENTS.md 根规则中对 slTerminal 无对象的条款**。pebrel 侧位置：`AGENTS.md` 全局规则——PR / Issue 广告禁令与 CONTRIBUTING.md 权威链；「不得声称加入 workflow 或 CODEOWNERS 即启用强制保护」；翻译零分配合同条款（i18n 决策归分片 06）；行数预算 / 门禁验证条款文本（机制归分片 11，条款文本不抄）。理由：无 PR 流程、无 CI、无贡献者生态，条款无对象。
2. **AGENTS.md 命名与加载机制**。pebrel 侧位置：全仓 `*/AGENTS.md` 分层入口清单。理由：CLAUDE.md 渐进披露为 harness 原生机制且与 AGENTS.md 同构，统一保留 CLAUDE.md 命名，不引入双轨文档。
3. **project-constraints.md 独立文件**。pebrel 侧位置：`docs/project-constraints.md`。理由：合同内容（行数硬限 / 提示线、依赖方向、禁抬预算删测试过门）已由分片 11 的 file-budgets.txt + dependencies.toml + check_architecture.py 机械承载，文字条款进 CLAUDE.md 相应节；独立文件与 CLAUDE.md 双源必腐。
4. **engineering-evidence.md 独立文件**。pebrel 侧位置：`docs/engineering-evidence.md`。理由：外部规范依据审查表是 pebrel 治理自证文档，slTerminal 无此传统；「决策须有一手证据」由 note 的 Evidence 段与 ADR 上下文段承接，不建独立审查表。
5. **pebrel docs/ 目录整体形态**。pebrel 侧位置：`docs/`（internationalization.md、lua-configuration.md、scientific-rendering-*、preview-release-checklist.md、skills/、screenshots/ 等）。理由：具体文档随对应功能裁定（归分片 04 / 06 / 08）；既定文档面 = CLAUDE.md 系 + adr.md + CONTEXT.md + test-exemptions.md + notes + release-notes + 合规文件，不重建 pebrel 式 docs 大杂院；「docs 只记核验事实」条款已采纳（采纳点 4），载体形态不抄。
6. **安装器附加集成任务与旧安装迁移**。pebrel 侧位置：`scripts/installer.iss` 的 `[Tasks]` / `[CustomMessages]` / `[Registry]`（installfont 字体安装、addtopath PATH 注入、WSL / OpenIn 右键菜单及冲突处理）与 `scripts/installer-migration.iss` 全套（旧安装探测 / 目录迁移 / 旧卸载链调 / 迁移失败兜底文案）。理由：逐项按定位裁决——slTerminal 无捆绑字体计划（字体策略归分片 06）、终端模拟器非命令行工具无需 PATH、右键菜单与 WSL 子菜单首版不做登记为后续增强；迁移逻辑无对象（slTerminal 无改名前身的安装基）。桌面快捷方式 / 登录自启两个可选项任务保留（默认不勾）。
7. **多平台 / 多形态打包脚本**。pebrel 侧位置：`scripts/package-linux.sh`、`package-macos.sh`、`package-msix.ps1`、`macos_dmg.py`、`android_release.py`、`generate-scoop-manifest.py` 及其测试（`test_macos_dmg.py`、`test_android_release.py` 等）。理由：既定不采纳 Linux / macOS / MSIX / Scoop / mobile；Windows 侧仅保留 build-windows-product.ps1 / build-installer.ps1 / package-release.ps1 / prepare-windows-runtime.ps1 同族四件。
8. **stable_release.py / preview_release.py 的 GitHub Release 编排面**。pebrel 侧位置：两脚本的 notes 生成 / 占位符替换（`STABLE_SHA256_PLACEHOLDER` / `PENDING FINAL BUILD`）/ 标签与 Release 创建 / 运行时报告收集（`RUNTIME_REPORTS`）。理由：slTerminal 无 CI、发布 = 本地构建 + 本机或另一台 PC 部署；只采纳资产校验纯逻辑（采纳点 12），编排不落地，未来开 GitHub Release 时以其为蓝本。
9. **`.agents/memory.md` 本地发布记忆**。pebrel 侧位置：`packaging/AGENTS.md` 与 `docs/release-notes/AGENTS.md` 引用的 `.agents/memory.md`（pebrel 侧 gitignore 的本地文件）。理由：「经真实发布核验的本地记忆」职责由 notes 体系 + 发布核验清单承接；不引入仓内第三记忆面（用户级 auto-memory MEMORY.md 与仓内 notes 已双轨）。
10. **历史名兼容与资产名沿革表**。pebrel 侧位置：`scripts/stable_release.py` 的 `expected_asset_names` 内旧名资产与 ARM64 版本递进表；`scripts/installer.iss` / `installer-migration.iss` 的 Nebula Terminal 相关文案。理由：历史兼容表对新 fork 无意义，slTerminal 资产名自 v0.x 起重登记（slterminal 单名）；旧名回流由禁名回流门禁防（分片 11）。
11. **根级 `.release-notes-*.md` 遗留位置**。pebrel 侧位置：仓库根 `.release-notes-0.9.0.md`。理由：历史遗留位置，release notes 统一归 `docs/release-notes/`，根级不建。
12. **MSIX / Scoop 更新归属记录**。pebrel 侧位置：`architecture/notes/packaging/2026-09-28-windows-distribution-ownership.md` 与 `packaging/AGENTS.md` 末条。理由：渠道不采纳（既定），归属记录无对象；其「打包成功 ≠ 渠道验收完成」的核验精神已由采纳点 11 承接。

## 优化方向

**文档治理终态（双轨 + 渐进披露）**。根 CLAUDE.md（全局规则 + 架构节 ownership 双列表 + 文档规范节）→ 各模块 / 目录 CLAUDE.md（模块细节真值，最近路径优先、不回填根文件）→ `.claude/adr.md`（当前生效决策的当前形态，原位收敛 + 沿革）与 `architecture/notes/`（成因档案与事故因果，不可改写、Supersedes 链、Revisit when 撤销条件）双轨并存互链；`CONTEXT.md` 术语表、`test-exemptions.md` 豁免登记、file-budgets 门禁（分片 11）各就其位。写决策的默认问句：「五年后还会被翻出来问责吗」——会则 note，仍是当前规则则 adr.md，能从代码自证则不写。

**打包终态（单入口 + 清单即合同）**。单一发布入口脚本串全链：全新构建 → 新鲜度核验 → 安装器编译 → 便携 zip → 预算钉测试 → SHA256 清单 → 发布核验清单打勾；Inno 为主、zip 为共享 manifest 下的便携副产物；staging 清单全等校验 + 安全清理边界 + 合规文件必含（LICENSE / THIRD-PARTY-NOTICES / licenses/ / 双语 CHANGELOG）内建；体积预算钉测试锚「预算值 + 防退化」，热路径 crate 清单随架构演进重登记并重跑钉测试。

**合规终态（GPL-3.0 四落点 + 随包合规族）**。LICENSE / Cargo.toml license / package.json / notices 四落点一次切换到位；逐组件归属声明（上游、版本、许可证、派生与改编内容、fork 基线与补丁位置）随依赖演进维护；ConPTY 来源声明与 NuGet 版本 pin 随捆绑流程走。

**发布节奏（无 CI 的本机纪律）**。发版 = 完成双语 CHANGELOG 与版本 notes → 入口脚本全链 → 人工核验清单（产物名 / 大小 / SHA256 / 版本回读）→ 本机与另一台 PC 双点验收，结论落 notes；release notes 只写已交付且有证据的用户可感知变化，开放问题标明范围。

**协调项**。分片 11「发布相关测试不随 fork」条随本 spec 定稿回改为「预算钉测试 + 安装器自测随迁（裁定见分片 12）」；file-budgets / dependencies.toml / 禁名清单 / notes 规则 / 打包脚本五处架构契约文件同批建立，保证 fork 初期每道门禁齐备「规则文本 + 机械检查 + 自测」三件套。
