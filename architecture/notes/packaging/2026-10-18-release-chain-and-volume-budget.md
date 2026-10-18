# 发布链、版本线与体积预算校准

## Status

已执行（2026-10-18，M11.1 打包链 + M11.2 体积校准）。当前生效规则见 `.claude/adr.md` ADR-0035。

## Context

项目无 CI，发布链必须全部本地可跑；安装器（Inno setup.exe）与便携 zip 必须来自同一构建与同一 feature 图。体积预算若只登记数值而无防退化机制，后续依赖增长会静默吃掉发布包体积。版本线与 Inno AppId 在首发后即成用户侧不可逆标识。

## Evidence

- 2026-10-18 全量功能态 release 实测（`[profile.release]` 冷代码 `opt-level="s"` + 热路径 O3）：setup.exe 17,899,557 字节（17.07 MiB），便携 zip 23,283,760 字节（22.21 MiB）。同 profile 未启用热路径 O3 时为 15.12 MiB / 19.29 MiB。
- 预算按「实测值 + 15%」钉为 setup 19.63 MiB / zip 25.54 MiB，写入 `scripts/tests/package-release.tests.ps1` 注释；数值、校准日期、锚定对象同处登记。
- `scripts/release.ps1` 实际编排 = 共享构建 → 安装器 → 便携 zip → 钉测试（存在时）→ `verify-release`，逐失败即停。
- 新鲜度判定基准 = 源码最新 mtime，而非脚本运行时刻。历史事故（2026-08-18，上游仓库）：修复已写入源码但未重链接，包里仍是前一晚的 exe。
- 已知边界（B72）：仅修改根 `Cargo.toml` 注释会让新鲜度检查假红——编译输入未变，cargo 不重链接；本次以 `cargo clean` 绕过。根治需改为「参与二进制的输入摘要」判据。

## Decision

- 版本线续 0.3.0；更新链对 `< 0.3.0` 一律视为需手动重装。
- Inno 生产 AppId `{D9D92995-C831-4610-87BC-4B82B43B3E71}`、夹具 AppId `{45EED1CD-AE5E-4D5C-84F2-8D1949722E9E}` 永不变更；夹具模式隔离 AppId/AppName/输出名/注册表键。
- 产物命名固定为 `slTerminal-v<version>-windows-x64-setup.exe` / `slTerminal-v<version>-windows-x64.zip`，`verify-release.ps1` 生成 `SHA256SUMS`。
- 体积门禁 = 预算值 + 钉测试；超预算即红，收紧预算永远合法，放宽须在提交信息说明原因。
- 发布脚本统一 `#Requires -Version 7`；正式路径禁止 `-SkipBuild`/`-AllowStale`。

## Rejected alternatives

- 保留旧单 exe 裸打 zip 链路：无清单、无合规文件、无新鲜度检查。
- 不隔离 `CARGO_TARGET_DIR`：会打死正在运行的实例（os error 5）。
- 以运行开始时刻作新鲜度基准：cargo 对未变更目标不重链接，会漏判旧二进制。
- 首发重计 1.0.0/0.1.0：与 0.2.0 用户感知割裂，且更新链仍需处理旧安装基。

## Consequences

- 发布前必须完成 CHANGELOG 对应版本小节（安装器与 zip 内置 CHANGELOG）。
- 改动 `[profile.release]` 体积相关数值或热路径钉包清单必须重跑 `scripts/tests/package-release.tests.ps1`。
- AppId、版本线、资产命名是发布后不可逆面；变更须走重大决策记录。
- B72 未闭环前，仅改根清单注释可能触发发布链假红。

## Validation

- `powershell -File scripts/tests/package-release.tests.ps1`：profile 钉值、清单、新鲜度、体积预算全绿。
- `powershell -File scripts/release.ps1 -Version 0.3.0`：产出 setup.exe + zip + SHA256SUMS（M11.1 实测）。
- `scripts/release-checklist.md` 人工核验（M11.4 首发全勾）。

## Supersedes

None。

## Revisit when

- 热路径钉包清单或 `[profile.release]` 体积相关数值变更时（必须重跑钉测试）。
- B72 治理落地（输入摘要判据）时，更新本 note 的新鲜度 Evidence。
- 出现 arm64 构建需求，或 GPUI 上游 arm64 支持变化时（iss 已保留 arm64 参数化形态）。
- 旧客户端接入诉求出现，需要重新评估 `legacy-attach` 时（该决策见 ADR-0029，与发布形态联动）。
