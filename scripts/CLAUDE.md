# 脚本族:构建、打包与门禁

## 决策与红线

- **单入口发布**:正式发布只走 `release.ps1`(共享构建 → 安装器 → 便携
  zip → 钉测试(存在时)→ `verify-release`),逐失败即停;手工分步仅用于
  定位。
- **发布构建纪律**:正式路径禁止 `-SkipBuild` / `-AllowStale`;两者只允许
  出现在脚本自测与发布彩排,且必须在命令中显式出现。发布包必须来自
  全新构建。
- **清单即合同**:包内文件集唯一真值源 = `build-windows-product.ps1` 的
  `$script:PackageEntries`;`package-release.ps1` 的 staging/zip 全等校验
  与 `installer.iss` 的 `[Files]` 必须与它同步,多一件少一件都红。
- **新鲜度判据**:二进制 mtime 必须 ≥ 全部成员 crate 与根清单的最新
  `.rs`/`.toml` mtime。基准是源码而不是脚本运行时刻(cargo 对未变更
  目标不重链接)。
- **版本零硬编码**:版本缺省回读 `slterm_app/Cargo.toml`;`installer.iss`
  的所有版本/布局参数经 ISCC `/D` 注入。产物命名合同:
  `slTerminal-v<version>-windows-x64-setup.exe` /
  `slTerminal-v<version>-windows-x64.zip`,`verify-release.ps1` 生成
  `SHA256SUMS`。
- **isl pin**:`ChineseSimplified.isl` 固定上游提交 + SHA256;缺失或哈希
  不符才下载,下载后仍不符则拒绝构建。
- **GUID 永不变更**:`installer.iss` 的生产 AppId 与夹具 GUID 一经钉死
  即成为续装/升级/卸载标识;夹具模式(`AcceptanceFixture`)必须与生产
  AppId、AppName、输出名、注册表键隔离。
- **凭据红线(SEC-18)**:脚本、iss、夹具中禁止出现任何真实 token/key。
- **禁名门禁**:本目录所有脚本与 iss 内的产品名只允许 `slTerminal` /
  `slterm`;提交前跑 `python scripts/check_brand_regress.py staged`。
- **PowerShell 7**:本目录全部 `.ps1` 首行声明 `#Requires -Version 7`;
  5.1 下不保证解析语义,报错应明确指向版本而非晦涩语法错误。

## 文件职责

- `build-windows-product.ps1`:共享显式构建(CARGO_TARGET_DIR 隔离)+
  清单表 + 新鲜度函数;`-LibraryOnly` 供其余脚本复用。
- `build-installer.ps1`:新鲜度/版本回读/isl pin/ISCC 探测 → setup.exe。
- `package-release.ps1`:staging 全等校验 + 安全清理边界 + 原子替换 → zip。
- `installer.iss`:全参数化安装器定义。
- `verify-release.ps1`:资产名集合 + 1 MiB 下限 + PE 魔数 + SHA256SUMS。
- `release.ps1`:薄编排入口。
- `release-checklist.md`:人工核验清单;命令成功不等于发布核验完成。

## 测试与门禁

- 钉测试(安装器参数化/清单/新鲜度/体积预算)归测试层脚本族,存在时由
  `release.ps1` 自动执行;单跑入口 `scripts/tests/package-release.tests.ps1`。
- 架构门禁:`python scripts/check_architecture.py`;禁名门禁:
  `python scripts/check_brand_regress.py staged`。
