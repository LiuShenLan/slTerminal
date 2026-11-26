# 脚本、CI、发布链、许可与生成资产审查

> 基线 `53c2a1504a287917f78e58f580a7bd4b3e752060`，只读审查当前工作树；范围含 `scripts/`、`.github/workflows/ci.yml`、`.githooks/`、`.cargo/config.toml`、`licenses/`、`THIRD-PARTY-NOTICES`、`assets/`、`tools/slterm-i18n-contract/`、发布文档与版本说明。

## 覆盖与局限

- 已覆盖：全部门禁脚本及其自测、安装器与便携包脚本、发布编排、运行时准备、资产预览生成器、Win32 输入矩阵脚本与基线、CI 工作流、git hooks、Cargo 构建配置、许可目录与生成清单、安装器文件表、Runtime schema 与控制面文档、数学语料、i18n 合同工具、发布说明与发布清单。
- 已对照：`docs/devPlan/plan.md`、`progress.md`、`traceability.md`，`docs/devPlan/design/dependencies.md`、`windows.md`、`runtime.md`，`docs/featureSpec` 的资产、验收、系统集成与 Runtime 合同，`.claude/docs/release/packaging.md`、`.claude/docs/operations/commands.md`、测试豁免表。
- 未覆盖：未运行 `cargo`、未重建安装器、未执行网络下载或真实安装，因此生成器逐字节一致、Inno 编译语义、真实卸载与签名链未在本审查中复现；相关结论只在读到代码或静态证据处标为确认。

## 问题

### 阻断｜内嵌 Maple Mono 字体缺少 OFL 许可与版权文本，许可审计完全不覆盖

- 置信度：确认
- 位置：`slterm_app/assets/fonts/MapleMonoNormal-NF-CN-Regular.ttf`；`slterm_app/src/app_shell/assets.rs` 的 `REQUIRED_FONT_BYTES`；`scripts/build-windows-product.ps1` 的 `PackageEntries`；`scripts/installer.iss` 的 `[Files]`；`THIRD-PARTY-NOTICES`；`scripts/check_licenses.py` 的 `NATIVE_DOCUMENTS`。
- 现象：字体经 `include_bytes!` 编译进 `slterm.exe` 并随安装器与便携包分发，但仓库内没有任何 OFL 文本、该字体的版权声明或独立许可文件；发布包的 `licenses/` 不含该字体许可，`THIRD-PARTY-NOTICES` 只声明 Latin Modern Math。
- 根因：许可核验只沿 Cargo 生产依赖图与已登记的原生源文本生成，未把内嵌字体、ConPTY 二进制等非 crate 资产纳入审计；手工 notice 也没有机械校验。
- 影响：字体名称表自述为 SIL Open Font License 1.1，要求随副本附带版权与许可文本；当前二进制再分发缺少这些文本，违反项目“第三方再分发义务必须机械闭合”的硬约束，而发布许可门禁仍会通过。
- 证据：字体名称表含 “Copyright 2022 The Maple Mono Project Authors” 与 “This Font Software is licensed under the SIL Open Font License, Version 1.1”；对已跟踪文件的全文检索找不到 Maple 或 OFL 文本；`NATIVE_DOCUMENTS` 只有 dav1d 与 SQLite；`PackageEntries` 与安装器文件表均无该字体的许可条目；`check_licenses.py` 只校验生成的 crate notice，不扫描 `assets/fonts`。
- 优化方向：把字体及同类内嵌第三方资产纳入许可生成与打包清单，随包提供完整 OFL 文本与版权行；许可门禁应以资产清单而不是 Cargo 图作为唯一覆盖面，避免再出现“代码在包内、许可不在包内”的漂移。

### 阻断｜发布说明哈希与当前 `dist` 及打包证据不一致，发布核验无法闭合

- 置信度：确认
- 位置：`release-notes/v0.3.0.md` 的 SHA256 块；`.claude/docs/release/packaging.md` 的验收证据；`scripts/verify-release.ps1` 的 `SHA256SUMS` 生成；`scripts/release-checklist.md` 的“构建与资产”“更新链”章节。
- 现象：当前 `dist/SHA256SUMS` 的安装器哈希为 `137d2a…`、便携包哈希为 `a8a19f…`；`release-notes/v0.3.0.md` 仍写 `e041…` 与 `ee97…`；`packaging.md` 同版本先后记录多套彼此不同的哈希。当前 `dist` 的体积又对应 `packaging.md` 最新一次的实测口径，说明产物已重建而版本说明与证据没有同步。
- 根因：版本说明的哈希块是手工维护的“预期条目”；`verify-release.ps1` 只写 `dist/SHA256SUMS`，没有回写版本说明或发布正文的通道；发布清单又要求格式不同的版本说明块与校验文件“逐字节一致”，本身不可执行。
- 影响：按当前版本说明发布时，更新链从发布正文解析的 `sha256` 会与资产实际哈希不符，更新被拒绝或发布错误声明；`release-notes` 的“SHA256 禁手抄”纪律与 `progress.md` 的发布验收结论同时失效。
- 证据：对当前 `dist` 各资产复算的 SHA256 与 `release-notes/v0.3.0.md` 所列值不同；`verify-release.ps1` 仅对 `$expected` 生成 `SHA256SUMS`；发布清单要求版本说明哈希块与 `SHA256SUMS` 逐字节一致，但前者是 Markdown 列表、后者是 `hash  filename` 行；`slterm_app/src/update_check.rs` 的 `checksum_from_release_body` 直接消费发布正文中的哈希块。
- 优化方向：让 SHA256 单一来源驱动发布说明与 `SHA256SUMS`，由发布链在产物确定后生成或校验版本说明；人工清单只核对机器可比的同一表示，不要求格式不同的内容逐字节相等；历史证据必须标日期与构建身份，避免旧哈希与当前产物混读。

### 重要｜许可审计根包名与实际 workspace 包名不一致，hook 生产图从未进入审计

- 置信度：确认
- 位置：`scripts/check_licenses.py` 的 `ROOT_PACKAGES` 与 `production_packages`；`slterm_hook/Cargo.toml` 的 `[package] name`；`scripts/tests/test_check_licenses.py` 的 `metadata` 夹具。
- 现象：实际包名是 `slterm_hook`，审计根集合写的是 `slterm-hook`。`production_packages` 只找到应用根，找不到 hook 根也不会报错，因为“缺少应用或 hook 根包”只在集合完全为空时触发；单测夹具也使用 `slterm-hook`，因此长期绿。
- 根因：根包名以字符串字面量维护，未从 `cargo metadata` 的 workspace 成员推导；单测使用与真实布局不同的包名，只验证了模拟数据。
- 影响：hook 的生产依赖图被静默跳过。当前 hook 零生产依赖，所以没有即时缺失文本；但零依赖策略一旦调整，新增依赖的许可会漏进包内而不被门禁发现，审计合同已失效。
- 证据：`slterm_hook/Cargo.toml` 声明 `name = "slterm_hook"`；`check_licenses.py` 的根集合为 `{"slterm", "slterm-hook"}`；`THIRD-PARTY-CRATE-NOTICES` 中没有 hook 根；测试夹具把根包命名为 `slterm-hook` 并放入 `workspace_members`。
- 优化方向：从 metadata 推导全部 workspace 根或至少使用真实包名并断言所有根均被解析；单测应使用真实清单名称，避免模拟名掩盖生产差异。

### 重要｜手工 `THIRD-PARTY-NOTICES` 缺版权行且引用不存在的路径，且没有门禁校验

- 置信度：确认
- 位置：`THIRD-PARTY-NOTICES` 的 ConPTY、Tabby、Herdr 与 SSH Bash 段落；`slterm_terminal/vendor/conpty/LICENSE`；`scripts/build-windows-product.ps1` 的 `PackageEntries`；`scripts/check_licenses.py` 的 notice 生成范围。
- 现象：ConPTY 段落只声明 MIT 并给出一段无版权行的 MIT 文本，未复现 Microsoft 版权行；随包清单不包含 `slterm_terminal/vendor/conpty/LICENSE`。Tabby 段落同样没有版权行。Herdr 与 SSH Bash 段落分别引用 `slterm_app/src/agent_detection` 和 `slterm_app/res/shell/bashrc`，这些路径都不存在。
- 根因：外层 notice 由人工维护，许可门禁只校验生成的 crate notice 与已登记的原生文档，不校验外层 notice 的路径、版权行或与真实资产的一致性。
- 影响：随包第三方二进制与图标的必要版权归属缺失，复用已删除代码的归因段落误导审计；许可证义务与来源声明都无法从包内文本闭合。
- 证据：`git ls-files` 不含 `slterm_app/src/agent_detection` 或 `slterm_app/res/shell/bashrc`；`THIRD-PARTY-NOTICES` 的 MIT 文本块没有 Microsoft/Tabby 版权行；`vendor/conpty/LICENSE` 含 Microsoft 版权但未出现在 `PackageEntries` 或安装器 `[Files]`；`check_licenses.py` 不读取外层 notice。
- 优化方向：把外层 notice 的每条归因与真实文件/资产绑定并纳入机械检查；随包二进制与品牌图标的版权行、许可文本必须与资产同源；删除失效归因或改写为当前真实路径。

### 重要｜dav1d 原生源从可变 tag 克隆，审计 commit 未进入构建

- 置信度：确认
- 位置：`vendor/dav1d-sys/build.rs` 的 `build_from_src`、`REPO`、`TAG`；`.cargo/config.toml` 的静态内部构建配置；`docs/devPlan/design/dependencies.md` 的 dav1d 审计锚。
- 现象：干净构建时执行 `git clone --depth 1 -b 1.5.0 <repo>`，已有目录时 `fetch` 后 checkout `FETCH_HEAD`；全过程不校验 commit，也不校验远端 ref 是否仍指向审计锚。设计文档记录的审计 commit 只存在于文档。
- 根因：沿用了上游 `dav1d-sys` 的 tag 构建方式，仓内补丁只加 `--vsenv`；没有把“可复现构建”的要求落实到原生源获取。
- 影响：远端 tag 被移动或仓库被替换时，发布二进制中的原生解码器会改变而 `Cargo.lock` 不变；离线构建不可行，审计锚无法证明最后产物对应的源码。
- 证据：`build.rs` 的 `REPO`/`TAG` 与 `git clone`、`git fetch`、`git checkout` 调用；设计文档要求审计 `1.5.0` 的固定 commit；CI 只固定 Meson/Ninja/NASM/pkg-config 版本，未固定或校验该原生源 commit。
- 优化方向：在拉取后校验固定 commit 或哈希，或把原生源做成仓内可校验的快照；让审计锚成为构建输入而非文档事实；同时记录本地发布机工具版本，否则“可复现”只覆盖 Rust 依赖。

### 重要｜构建新鲜度摘要漏掉非 `.rs`/`.toml` 构建输入，`-SkipBuild`/彩排路径可带旧产物过门禁

- 置信度：确认
- 位置：`scripts/build-windows-product.ps1` 的 `SourceRoots`、`Get-SourceDigest`、`Get-NewestSourceTime`、`Assert-FreshBinaries`；`scripts/tests/package-release.tests.ps1` 的 `Invoke-FreshnessSelfTest`。
- 现象：摘要只对 `.rs`/`.toml` 取路径与内容哈希。实际参与编译的 `vendor/conpty` 二进制、`vendor/tree-sitter-json5` 与 `vendor/tree-sitter-dockerfile` 源码、`slterm_app/assets/fonts`、`slterm_app/i18n` JSON、`slterm_app/res/hooks`、`handoff.ps1`、`shell-integration.ps1`、`assets/runtime-api-v2.schema.json` 与 `rust-toolchain.toml` 都不进入摘要；其中 vendor 补丁根甚至未登记进 `SourceRoots`。
- 根因：新鲜度判据按“源码文件后缀”而非完整构建输入定义，测试夹具也只使用 `.rs`，无法暴露缺口。
- 影响：仅修改字体、i18n、hook 脚本、嵌入 schema 或内嵌 ConPTY 后，若使用 `-SkipBuild` 或已有构建戳的目录，打包仍会通过新鲜度检查，发布包可能携带旧二进制；这与脚本文档“新增构建输入必须登记 SourceRoots”相矛盾。
- 证据：`include_bytes!`/`include_str!` 的调用点覆盖上述资产；`Get-SourceDigest` 只枚举 `*.rs`、`*.toml`；`SourceRoots` 不含 `vendor/tree-sitter-*`，且 `vendor/conpty` 即使作为根也不会被后缀过滤到；`Invoke-FreshnessSelfTest` 只造 `.rs` 夹具。
- 优化方向：以 Cargo 的 `dep-info` 或显式资产清单覆盖全部构建输入，而非按后缀猜测；把非 `.rs` 输入加入自测反面用例，使“改资产不重建”必定红。

### 重要｜Runtime schema 生成器漂移测试从未进入 CI 或发布链

- 置信度：确认
- 位置：`slterm_runtime_protocol/Cargo.toml` 的 `schema` feature；`slterm_runtime_protocol/src/protocol_tests.rs` 的 `generated_schema_matches_the_packaged_asset`；`.github/workflows/ci.yml`；`scripts/release.ps1`；`assets/runtime-control-api.md` 的操作命令。
- 现象：唯一逐字节比较生成 schema 与随包资产的测试受 `#[cfg(feature = "schema")]` 门控；CI 的所有 cargo 步骤均未传 `--features schema`，发布链也不传。应用侧测试只验证服务器对随包字节计算的哈希，不验证随包字节是否仍等于生成器输出。
- 根因：schema 生成器与资产同源校验被当作可选开发命令，没有接入任何常挂门禁。
- 影响：修改 DTO 或 schema 生成逻辑后，CI 仍绿，随包 `runtime-api-v2.schema.json` 可漂移；应用把旧 schema 编进二进制，`runtime.hello` 的 schema 哈希与客户端生成的合同不一致，违反“生成资产与生成器同源”。
- 证据：`schema` 非默认 feature；CI 的测试与 clippy 命令均无该 feature；`release.ps1` 只调用构建、打包与核验，没有 schema 步骤；`assets/runtime-control-api.md` 把定向测试写成人工命令；`slterm_app/src/local_control/service_tests.rs` 只对 `include_bytes!` 的资产求哈希。
- 优化方向：把 schema 逐字节比较纳入 CI 与发布前门禁，或在构建脚本中生成并比对；随包资产与生成器必须由同一命令产出。

### 重要｜随包 ConPTY 运行时的校验与实际嵌入二进制脱节，默认测试还跳过架构修复

- 置信度：确认
- 位置：`scripts/prepare-windows-runtime.ps1`；`scripts/tests/windows-runtime.tests.ps1`；`slterm_terminal/src/tty/windows/conpty.rs` 的 `CONPTY_DLL_BYTES`、`OPENCONSOLE_EXE_BYTES`；`scripts/release.ps1` 的钉测试循环；`.github/workflows/ci.yml`；`THIRD-PARTY-NOTICES` 的 ConPTY 段落。
- 现象：产品实际嵌入的是 `slterm_terminal/vendor/conpty` 下的二进制；`prepare-windows-runtime.ps1` 的产物不被构建、打包或发布链引用，`windows-runtime.tests.ps1` 只测试准备脚本下载与修复逻辑。发布链与 CI 都不运行该测试；即使手工运行，未传 `-ArchivePath` 时错误架构修复分支会被静默跳过。
- 根因：把“运行时准备脚本的测试”误当成“随包运行时的校验”，没有对 vendored 二进制建立哈希断言；测试用可选参数控制覆盖，却没有打印跳过或登记豁免。
- 影响：替换或损坏 `vendor/conpty` 文件、版本与 `prepare-windows-runtime.ps1` 的 pin 漂移都不会被任何门禁发现；`THIRD-PARTY-NOTICES` 声称哈希由该脚本校验，实际发布产物未经过该脚本。默认命令还会让人误以为架构修复用例已通过。
- 证据：`conpty.rs` 对 `../../../vendor/conpty/*` 使用 `include_bytes!`；全仓检索只发现测试调用 `prepare-windows-runtime.ps1`；`release.ps1` 的测试列表不含 `windows-runtime.tests.ps1`；测试中其他架构修复段由 `if ($ArchivePath)` 包裹；`pty_integration_tests` 只复制文件对，不断言哈希。
- 优化方向：对随包 vendor 二进制建立固定哈希门禁，并让运行时测试直接消费这些文件；把运行时测试接入 CI 或发布链并提供归档夹具；若保留条件分支，必须显式报告跳过并登记豁免。

### 重要｜`release.ps1` 的钉测试只执行体积与 profile，安装器/清单/新鲜度自测实际不运行

- 置信度：确认
- 位置：`scripts/release.ps1` 的测试循环；`scripts/tests/package-release.tests.ps1` 的 `-SelfTest` 分支与默认分支；`.claude/docs/release/packaging.md` 的钉测试声明；`scripts/CLAUDE.md` 的验收段。
- 现象：发布链调用 `package-release.tests.ps1 -Version … -Directory …`，未传 `-SelfTest`；默认分支只做 profile 钉值、体积预算与缺件检查。清单、新鲜度、原生 dumpbin 夹具自测只在 CI 的 `-SelfTest` 步骤运行，发布链不运行。`installer.tests.ps1` 被循环探测但仓库不存在；`build-installer.ps1`、`package-release.ps1` 的 staging、安全清理、zip 二次读回、原子替换和 `verify-release.ps1` 都没有端到端测试。
- 根因：测试脚本按“体积钉测试”组织，`-SelfTest` 成为 CI 专用路径；发布文档却把整族测试描述为发布链自动执行。
- 影响：发布链对清单、新鲜度与安装器参数的回归没有实际拦截；文档的发布验收结论与执行面不一致，关键发布路径无自动化测试。
- 证据：`release.ps1` 的 `& $testPath -Version $Version -Directory $OutputDirectory`；`package-release.tests.ps1` 的默认路径只调用 `Assert-ReleaseProfile` 与 `Assert-ReleaseAssets`；`release.ps1` 的 `installer.tests.ps1` 路径不存在；仓库没有 `build-installer`/`package-release` 主路径测试。
- 优化方向：让发布链显式运行自测族或拆出发布专用测试；为安装器生成、staging 全等、清理边界、原子替换和 SHA256SUMS 复算建立可自动化失败注入用例。

### 重要｜“清单即合同”没有机械化全等检查，安装器文件表可独立漂移

- 置信度：确认
- 位置：`scripts/tests/package-release.tests.ps1` 的 `Invoke-ManifestSelfTest`；`scripts/build-windows-product.ps1` 的 `PackageEntries`、`PackageDirectories`；`scripts/installer.iss` 的 `[Files]`；`scripts/package-release.ps1` 的 `Assert-Manifest`。
- 现象：清单自测只检查 CLI 图标的通配行和部分许可行是否出现在安装器中，没有把安装器 `[Files]` 的完整集合与 `PackageEntries` 展开结果做全等比较；便携包侧的全等只对着共享清单自身。新增、删除或写错一条安装器条目时，自测仍可能通过。
- 根因：安装器脚本是声明式文本，测试只做了字符串包含断言，没有解析文件表；发布链的 `Assert-Manifest` 只验证 staging/zip 与共享清单，不读取安装器。
- 影响：安装器与便携包可出现文件集不一致，违反“多一件少一件都红”的外部合同；许可或文档缺失可只出现在一种产物中，直到人工安装才发现。
- 证据：`Invoke-ManifestSelfTest` 的断言覆盖 `assets/cli-svg` 通配与部分 notice；`package-release.ps1` 的 `Assert-Manifest` 只消费 `Resolve-PackageSources` 的结果；`installer.iss` 没有被任何脚本解析为清单集合。
- 优化方向：从 `PackageEntries` 派生安装器 `[Files]`，或在测试中解析安装器并做条目级全等；通配目录也按展开结果比较，避免只验证示例行。

### 一般｜CI 不执行许可清单核验，锁文件或清单变更只能在发布时爆红

- 置信度：确认
- 位置：`.github/workflows/ci.yml` 的 `windows-gates` 步骤；`scripts/check_licenses.py`；`.githooks/pre-commit`；`scripts/release.ps1` 的许可步骤。
- 现象：CI 与本地 pre-commit 都不运行 `check_licenses.py`；只有正式发布在构建前执行。`Cargo.toml`/`Cargo.lock` 变化合入时，生成 notice 是否过期不在提交门禁内。
- 根因：许可核验被归入发布链，未进入“提交门禁常挂”清单；`CLAUDE.md` 的门禁列表也不含许可。
- 影响：过期许可清单可在 CI 全绿后长期存在，直到发布机才失败；对“许可清单与 Cargo.lock 一致”的合同缺少前置反馈，且字体类缺口即使运行该脚本也发现不了。
- 证据：CI 工作流的 python 门禁只有架构、文档、禁名与 unittest；pre-commit 同样没有 `check_licenses.py`；`release.ps1` 是唯一调用点。
- 优化方向：把锁文件/依赖变更触发的许可核验纳入 CI，并与资产级许可审计合并；提交门禁至少提示 notice 过期，而不是留到发布。

### 一般｜CI 未按发布口径验证：无 `--locked`、无 release 构建、无 dumpbin 静态链接审计

- 置信度：确认
- 位置：`.github/workflows/ci.yml` 的 cargo 步骤；`scripts/build-windows-product.ps1` 的 `--locked` 构建与 `Assert-Dav1dStatic`；`.claude/docs/operations/commands.md` 的静态检查与测试命令。
- 现象：CI 的 clippy 与测试命令都没有 `--locked`；CI 从不构建 release 产品，也不运行 `dumpbin /dependents` 的静态链接审计。发布构建使用 `--locked`，且只有 `build-windows-product.ps1` 在 release 路径执行 `Assert-Dav1dStatic`。
- 根因：CI 被定位为提交测试工作流，发布构建只在本地执行；但依赖解析与静态链接是发布合同，未在 CI 预演。
- 影响：`Cargo.lock` 与清单漂移时 CI 可自动更新锁并保持绿，到发布 `--locked` 才失败；dav1d 动态依赖回归也只有在发布机才被发现。CI 安装的 dav1d 工具链因此只覆盖编译，不覆盖最终链接纪律。
- 证据：CI 的 `cargo clippy`/`cargo test` 命令无 `--locked`；`build-windows-product.ps1` 的 build 参数含 `--locked` 与 `--timings`；`Assert-Dav1dStatic` 只在共享构建末尾调用；CI 没有 `cargo build --release` 或 dumpbin 步骤。
- 优化方向：CI 增加 `--locked` 与一次 release 构建/静态链接审计，至少对 slterm 产品；把 release profile 的体积与链接门禁前移，减少发布机才暴露的失败。

### 一般｜架构门禁在 CI 与钩子中不使用 `--base`，预算/豁免增长不可见

- 置信度：确认
- 位置：`scripts/check_architecture.py` 的 `--base`；`.github/workflows/ci.yml` 的架构门禁步骤；`.githooks/pre-commit`；`architecture/file-budgets.txt`。
- 现象：`--base` 可拒绝线数上限提高、source root 删除和新/增大的 exception，但 CI 与 pre-commit 都只跑当前树检查。只要在当前树内改预算文件，所有常挂门禁都会通过。
- 根因：基线比较能力存在，却没有接入任何自动入口；历史债务棘轮只在手工运行时生效。
- 影响：新增豁免或放宽限额可以随普通提交合入，违反“新决策尽量不豁免”的纪律；门禁只能保证当前文件不超限，不能保证债务不增长。
- 证据：CI 步骤为 `python scripts/check_architecture.py`；pre-commit 同命令；`Budget.check` 中的 base 分支只在传入 `base` 时执行；仓库没有任何脚本或 CI 步骤传 `--base`。
- 优化方向：PR 工作流以目标分支作为 base 运行棘轮检查，或至少在提交门禁中比较 HEAD；让预算文件变更必须伴随基线证据。

### 一般｜版本校验与默认值跨脚本不一致，单独调用可生成不可核验产物

- 置信度：确认
- 位置：`scripts/release.ps1` 的版本校验；`scripts/verify-release.ps1` 的版本校验；`scripts/build-installer.ps1` 的版本正则与默认值；`scripts/package-release.ps1` 的版本正则与默认值；`scripts/CLAUDE.md` 的版本零硬编码纪律。
- 现象：`release.ps1` 与 `verify-release.ps1` 只接受三段纯数字；`build-installer.ps1` 额外接受预发布/构建元数据；`package-release.ps1` 接受任意字母数字版本且默认 `unreleased`。按脚本文档单独调用打包脚本会生成 `slTerminal-vunreleased-windows-x64.zip`，与“版本缺省回读 `slterm_app/Cargo.toml`”不一致，也无法被 `verify-release.ps1` 接受。
- 根因：版本合同分散在多个脚本，以各自用途的正则近似实现；没有共享的版本解析/命名模块。
- 影响：预发布或默认调用会产生命名、版本信息与核验规则不一致的资产；发布链以外的单步操作容易制造无法发布或无法核验的包。当前正式入口不受影响，但脚本作为独立工具的行为不可靠。
- 证据：各脚本的版本正则与默认值；`package-release.ps1` 默认 `unreleased`；`verify-release.ps1` 与 `release.ps1` 拒绝非三数值段；`build-installer.ps1` 的 `NumericVersion` 还额外截断/补零。
- 优化方向：使用单一版本来源与统一解析器，默认回读产品版本；所有产物命名、安装器版本与核验规则由同一契约派生，单步脚本要么拒绝不合规版本，要么明确进入彩排命名空间。

### 一般｜`cargo` 调用依赖调用者工作目录，绝对路径运行发布脚本会失败或构建错误工作区

- 置信度：确认
- 位置：`scripts/build-windows-product.ps1` 的 `& cargo @buildArgs`；`scripts/release.ps1` 的子脚本调用；`.claude/docs/operations/commands.md` 的发布命令。
- 现象：构建脚本不切换仓库根，也不传 `--manifest-path`；`cargo` 以调用者当前目录解析清单。按文档从仓库根运行正常，但用绝对路径从其他目录运行时，`cargo` 找不到清单；若当前目录恰有另一个清单，则可能构建错误项目，而后续检查仍指向仓库的 `target`。
- 根因：所有源路径都用 `$PSScriptRoot` 派生，唯独 `cargo` 调用沿用了进程 cwd。
- 影响：发布/彩排脚本的可用性依赖调用位置；最坏情况下构建对象与打包对象不一致，错误在后续版本回读或缺失产物处才暴露。
- 证据：`build-windows-product.ps1` 设置 `CARGO_TARGET_DIR` 但不改变工作目录，调用 `cargo` 时没有 `--manifest-path`；`release.ps1` 以绝对路径调用子脚本，子脚本也不切换目录；文档只写 `pwsh -File scripts/release.ps1`，未约束 cwd。
- 优化方向：构建入口显式切到仓库根或传 `--manifest-path`；所有子进程的工作目录由脚本拥有，不继承调用者。

### 一般｜发布产物无签名链，`verify-release` 只做 MZ 魔数与体积下限

- 置信度：确认
- 位置：`scripts/verify-release.ps1` 的 PE 检查与 `SHA256SUMS`；`scripts/release.ps1`；`.github/workflows/ci.yml`；`docs/featureSpec/acceptance.md` 的“更新与安装”边界；`slterm_app/src/update_check.rs`。
- 现象：发布链不调用 Authenticode 签名或验证工具，CI 也没有签名步骤；`verify-release.ps1` 只检查文件大于 1 MiB 且前两字节为 MZ，不检查证书、发布者或时间戳。更新链只校验发布正文中的 SHA256。
- 根因：发布合同把可安装性交给架构与哈希，签名被视为发布机外部步骤，但没有清单、脚本或人工项承载。
- 影响：安装器无签名会触发 SmartScreen 警告；哈希与资产同渠道分发，无法在渠道被篡改时独立证明来源；发布核验对“PE 文件”的判定可被任意 MZ 文件满足。
- 证据：`verify-release.ps1` 的检查只有长度与 MZ；仓库脚本与工作流没有 `signtool`/`Get-AuthenticodeSignature`/`WinVerifyTrust`；发布清单没有签名项；`update_check.rs` 只解析 SHA256。
- 优化方向：明确是否要求签名；若要求，把签名与验证作为发布链和人工清单的组成部分，并让更新链至少能验证发布者身份；若不要求，在发布边界文档中登记该风险，而不是让 `verify-release` 的 PE 检查暗示更强保证。

### 一般｜i18n 合同只约束目录占位符，不约束调用点参数

- 置信度：确认
- 位置：`slterm_app/build/i18n.rs` 的占位符校验；`slterm_app/src/i18n/mod.rs` 的 `UiLanguage::format`；`slterm_app/tests/i18n_contract.rs` 的 `resolved_locale_and_typed_formatting_follow_the_catalog`；`tools/slterm-i18n-contract`。
- 现象：生成器只验证各语言目录的占位符集合一致；运行时的 `format` 接受无类型的 `&[(&str, &str)]`，缺失参数会把 `{name}` 原样留在界面，额外参数被忽略。合同测试只抽查少量消息，类型系统也不把 `Message` 与所需占位符绑定。
- 根因：typed 目录只解决了键的枚举化，没有把占位符名字带入生成类型；调用点正确性靠人工审查。
- 影响：新增或修改带参消息时，CI 不能发现调用点传错/漏传占位符，用户会看到 `{error}` 一类原文；`tools/slterm-i18n-contract` 虽能独立运行同一合同，但不增加调用点覆盖。
- 证据：`format` 签名与 `format::substitute` 的行为；`i18n_contract.rs` 只对 `SettingsStatusOperationFailed` 与 `SettingsStatusResetBackup` 做格式化断言；生成器只输出 `Message` 枚举与查表，不输出占位符元数据。
- 优化方向：在生成物中携带每个消息的占位符集合，或生成按占位符命名的格式化入口，让漏传/错传在编译期失败；合同测试至少遍历全部带参消息。

### 一般｜发布文档的“已知边界”与当前实现不符

- 置信度：确认
- 位置：`.claude/docs/release/packaging.md` 的 2026-10-18 首发证据“已知边界”；`scripts/release-checklist.md` 的“干净环境验收”章节；`scripts/installer.iss` 的 `[UninstallRun]`、`[UninstallDelete]`、`DefaultGroupName`；`slterm_app/src/cli.rs` 的 `run_setup_ai`。
- 现象：文档仍称 `setup-ai --remove` 子命令不存在、卸载 hook 清理为 no-op；但 CLI 已实现该子命令并有解析与执行测试，安装器也把它作为 `[UninstallRun]`。文档仍称开始菜单快捷方式落在 `Programs\(Default)`，安装器却显式设置 `DefaultGroupName`；文档称更新锁文件可能导致目录保留，安装器已增加 `.slterm-update*` 的 `[UninstallDelete]`。
- 根因：发布证据按日期累积，旧的“已知边界”没有在后续修复后重审；文档策略只扫描编号与路径，不校验边界描述与当前实现一致。
- 影响：发布核验者会按错误边界放行或误判卸载行为；`release-checklist` 明确要求“不得误报为成功”，却基于一个已经不成立的“未实现”前提。
- 证据：`cli.rs` 的 `run_setup_ai`、`Cli` 子命令解析与安装器 `[UninstallRun]`；`installer.iss` 的 `DefaultGroupName` 与 `[UninstallDelete]`；`packaging.md` 与 `release-checklist.md` 的旧表述。
- 优化方向：把“已知边界”改为可复核的当前事实，并在修复落地时同步删除旧条目；发布清单的每项应指向可执行的观察点，而不是历史结论。

### 一般｜本地 hooks 可绕过，提交说明与无上游分支推送不在任何远端门禁内

- 置信度：确认
- 位置：`.githooks/pre-commit`、`.githooks/pre-push`、`.githooks/commit-msg`；`scripts/check_brand_regress.py` 的 `pending_push_range`；`.github/workflows/ci.yml` 的禁名步骤；`git ls-files -s` 的 hook 文件模式。
- 现象：hooks 需一次性 `core.hooksPath` 才生效，`--no-verify` 可跳过全部；`pre-push` 在分支无上游时打印跳过并返回成功，提交说明扫描只存在于本地 `commit-msg`；CI 只跑 `check_brand_regress.py full`，扫描文件内容不扫描提交说明。hook 文件以普通文件模式入库，POSIX 克隆不会执行。
- 根因：禁名纪律被设计成本地开发体验的一部分，远端 CI 没有覆盖提交说明与无上游首推；hook 的可执行位也未随仓库固定。
- 影响：旧品牌名可经 `--no-verify`、首个无上游推送或非 Windows 客户端进入历史；当前文件全量门禁仍绿，但历史与提交说明已污染。
- 证据：`pending_push_range` 无上游时返回 None 并退出 0；CI 禁名步骤为 `full`；`git ls-files -s` 显示 hook 为普通模式；`commit-msg` 不在 CI。
- 优化方向：在 CI 对 PR/推送范围执行禁名范围扫描（至少新增行），并把提交说明检查放在不可绕过的服务端流程；hook 文件模式在仓库内固定，文档说明启用是必需步骤而非可选。

### 一般｜数学语料引用不存在的测试、主题与截图，验收指令不可执行

- 置信度：确认
- 位置：`assets/corpus/math-rendering-test.md`；`slterm_app/src/scientific_corpus.rs` 的 `DOCUMENTS`；`scripts/generate-asset-previews.ps1`；`slterm_app/src/theme_library` 与 `slterm_settings` 的主题注册。
- 现象：语料要求运行 `native_reader_selection_preview` 测试并设置 `SLTERM_SELECTION_QA_*`，但全仓没有该测试；要求对照 `MintLight` 主题，但内置主题只有其他名称；引用 `screenshots/reader-selection-opacity.png`，`assets/corpus` 下没有该目录。语料仍被数学回归测试读取，只是这些文字与图片路径不参与公式断言。
- 根因：语料文档是历史验收材料的文本副本，后续主题/测试淘汰时没有同步；资产预览门禁只覆盖 `docs/featureSpec/assets`，不覆盖 `assets/corpus` 的相对链接与命令。
- 影响：人工验收者按语料执行会得到“测试不存在/主题不存在/图片缺失”的假失败；语料作为验收输入的可信度下降，且没有门禁阻止继续腐化。
- 证据：全仓检索 `native_reader_selection_preview` 只命中该语料；`ThemeName`/主题注册表没有 `MintLight`；`find assets/corpus` 只有 Markdown，没有 `screenshots`；`scientific_corpus.rs` 只解析数学事件，不校验图片或命令。
- 优化方向：把语料中的可执行指令与资产链接纳入校验，或删除不再维护的人工段落；测试与主题退役时同步修订语料，避免把历史验收稿当作当前可用步骤。

### 提示｜`verify-release` 的 SHA256SUMS 非原子写，安装器输出缺少失败清理

- 置信度：确认
- 位置：`scripts/verify-release.ps1` 的 `[IO.File]::WriteAllText($checksumsPath, …)`；`scripts/build-installer.ps1` 的 ISCC 直接输出 `$setupPath`；`scripts/package-release.ps1` 的 zip 原子替换作为对照。
- 现象：便携包采用 staging + 临时 zip + 同卷移动，失败会清理；但 `SHA256SUMS` 直接写目标文件，安装器由 ISCC 直接写 `dist` 的最终路径。写入中断可能留下截断的校验文件或半成品安装器。
- 根因：原子写纪律只落实到 zip，没有统一到发布产物。
- 影响：中断后 `dist` 可能出现半文件；`verify-release` 的下一次运行可能把半成品当作资产，人工复制发布说明时也可能读到截断校验。当前发布链逐失败即停，风险有限，但合同要求原子导出。
- 证据：`verify-release.ps1` 的写调用没有临时文件与替换；`build-installer.ps1` 只检查 ISCC 退出后产物存在，没有失败清理；`package-release.ps1` 的 `[IO.File]::Move` 与 `Remove-StageSafely` 显示已有更严格模式。
- 优化方向：统一所有发布产物的临时文件 + 同卷替换与失败清理；SHA256SUMS 在全部校验通过后再原子落盘，并对半成品做清理。

### 提示｜文档策略与预算扫描的后缀覆盖不完整，部分脚本族不受门禁

- 置信度：确认
- 位置：`scripts/check_doc_policy.py` 的 `CODE_SUFFIXES`；`scripts/architecture/budgets.py` 的 `SOURCE_SUFFIXES`；`.githooks` 的无扩展名脚本；`scripts/win32_input_matrix_probe.cjs`；`scripts/installer.iss`。
- 现象：文档策略只扫描 `.rs`、`.py`、`.ps1`、`.toml`、`.iss`；工作流 `.yml`、shell hook 与 `.cjs` 探针不在范围内。行预算只统计若干后缀，`.cjs` 与 `.iss` 不计数。无扩展名的 `.githooks` 脚本因此不被文档策略扫描。
- 根因：后缀表按早期脚本族维护，新增 `.cjs`、工作流与无扩展名脚本时没有同步。
- 影响：过程编号、行号引用或失效路径可以进入 YAML、shell hook 与探针而不被门禁拦截；行预算对部分脚本失真。当前内容未触发，但覆盖面与“脚本门禁”声明不完全一致。
- 证据：后缀表；`.githooks` 文件名的后缀为空；`win32_input_matrix_probe.cjs` 与 `installer.iss` 的实际后缀；CI 只调用上述门禁。
- 优化方向：以仓库内脚本/资产的显式清单维护扫描面，新增脚本类型时同步登记；对无扩展名的 hook 脚本按路径或 shebang 识别，而不是依赖后缀。

### 提示｜Win32 输入矩阵未进入命令矩阵，错误处理与路径引用偏弱

- 置信度：确认
- 位置：`scripts/win32_input_matrix.ps1`；`scripts/win32_input_matrix_probe.cjs`；`.claude/docs/operations/commands.md`；`.claude/docs/testing/exemptions.md`。
- 现象：命令矩阵未列出该脚本；脚本设置 `$ErrorActionPreference = 'Continue'`，`Start-Process` 失败或窗口句柄为零时不会立即中止，只在后续比对失败；`-ArgumentList` 以数组拼接路径，仓库或探针路径含空格时工作目录/日志参数可能被拆开；脚本也不检查 `node` 是否存在。
- 根因：脚本保持早期手工验证形态，未按发布/门禁脚本的错误处理与路径纪律收敛。
- 影响：手工执行可能得到晦涩的“基线不匹配”而非启动失败；路径含空格的环境不可用；新人无法从命令矩阵发现该回归入口。豁免表只登记了 CHORD 不可自动化，未登记脚本本身的运行前提。
- 证据：脚本的 `$ErrorActionPreference`、`Start-Process`/`MainWindowHandle` 流程、`-ArgumentList` 数组；`commands.md` 无该命令；`exemptions.md` 只覆盖真实 CHORD。
- 优化方向：按其他脚本的失败即停与显式前置检查收敛；对含空格路径使用显式参数文件或 `ProcessStartInfo`；在命令矩阵与豁免表登记运行前提和覆盖边界。

### 提示｜CI 关闭了 `-D warnings`，clippy 只拦错误不拦警告

- 置信度：确认
- 位置：`.github/workflows/ci.yml` 的工具链步骤 `rustflags: ""`；`cargo clippy` 步骤；`.claude/CLAUDE.md` 的“门禁常挂”静态检查条目。
- 现象：工作流显式清空 action 默认的 `-D warnings`，clippy 命令本身也没有 `-- -D warnings`。因此项目自身 clippy 警告不会让 CI 变红。
- 根因：为规避依赖/build script 的第三方警告而整体清空 rustflags，没有把警告限定到第一方 crate。
- 影响：“clippy 门禁全绿”只代表无错误；第一方警告可随提交合入，静态检查口径与文档暗示不一致。当前是否有警告未在本审查中运行验证。
- 证据：CI 的工具链输入与 clippy 命令；仓库没有 `[lints]`/`deny(warnings)` 作为替代；工作流注释明确说明该输入会改变 clippy 口径。
- 优化方向：用 `-- -D warnings` 或 clippy lint 配置只约束工作区成员，保留第三方依赖的告警可见性；若确实接受警告，在文档中写成“clippy 错误门禁”而不是“clippy 全绿”。

### 提示｜发布纪律文本与实现口径不一致：`-SkipBuild` 与“全新构建”

- 置信度：确认
- 位置：`scripts/CLAUDE.md` 的“发布构建纪律”；`.claude/docs/release/packaging.md` 的“共享构建与 feature 图”；`scripts/release.ps1` 的子脚本调用；`scripts/build-windows-product.ps1` 的默认 `TargetDirectory`。
- 现象：文档称正式路径禁止 `-SkipBuild`/`-AllowStale`，且不得从已有 `target/` 复用产物；但 `release.ps1` 自身向子脚本传 `-SkipBuild`，默认构建目录又是仓内 `target`，`cargo build` 会增量复用已有构建。文档描述的是“用户不得跳步”的意图，代码没有形成可核验的“全新构建”约束。
- 根因：共享构建的目标与“不得复用产物”的表述没有统一；`-SkipBuild` 同时承担“发布编排内部去重”和“外部跳步”两种语义。
- 影响：发布产物不一定由本次发布尝试从空目录生成；若构建缓存或环境异常，`verify-release` 仍会接受产物。脚本纪律文本也无法作为审核依据。
- 证据：`release.ps1` 的 `-SkipBuild` 参数；`build-windows-product.ps1` 的 `DefaultTargetDirectory` 与 `cargo build`；`packaging.md` 的“不得从已有 target/ 或旧 dist/ 复用产物”；没有清空 target 或独立发布目录的步骤。
- 优化方向：区分“编排内部共享构建”和“外部跳步”，用独立构建目录或显式 clean 证据落实全新构建；文档改写成可执行判据，而不是禁止一个内部参数。

## 跨域观察

- 更新链在 `slterm_app/src/update_check.rs` 的 `checksum_from_release_body` 中直接解析发布正文的 SHA256 块；前述发布说明哈希问题会直接变成更新失败，修复版本说明时必须保持该块的解析格式。
- `slterm_app/src/app_shell/assets.rs` 的字体注册和 `slterm_terminal` 的 ConPTY 嵌入都属于“代码内资产”，但许可与新鲜度门禁都按文件类型/依赖图组织；同类缺口会继续出现在任何新的嵌入资产上。
- `tools/slterm-i18n-contract` 不被 CI 或命令矩阵引用；根 workspace 的同名合同测试也覆盖生成器，但该工具作为“脱离 GPUI 的独立运行入口”没有被验证，工具自身的构建脚本文档也没有维护入口。

## 存疑未证

- Inno Setup 的 `AppId={{#AppIdValue}` 预处理器与转义展开未在本审查中通过 ISCC 编译验证。需要一份编译后安装器的 AppId/卸载注册表证据；若展开为字面 `{#AppIdValue}`，续装/升级/卸载身份会与夹具或历史安装不一致。
- `[UninstallRun]` 的 `setup-ai --remove` 返回非零时，Inno 是否继续卸载且不向用户报告失败，需要一次真实卸载失败注入确认；代码层只能确认清理失败会返回非零，安装器脚本没有消费该结果。
- `build-installer.ps1` 在 ISCC 失败时是否由 Inno 自动清理半成品、以及是否可能覆盖上一份可用安装器，需要故障注入或 ISCC 文档/实测；当前只能确认脚本没有显式清理。
- 生成 schema 与随包资产的逐字节一致性、许可清单与当前 `Cargo.lock` 的完整一致性未在本审查中运行生成器/`cargo metadata` 验证；只做了静态路径与抽样版本核对。
- 当前 `dist` 产物的构建来源只能从体积与现有证据推断为最新一次回归构建，无法从文件本身证明对应的源码 commit 与工具链；发布前置条件仍需要发布机重新生成并记录。
