# Phase 2 修复执行编排参数（execution-plan）

> 通用执行规则不复制——单一真值源在 `/systematic-changes-execute`。本文件只写任务特定编排参数。
> 清单：`docs/review-phase2-fix/checklist.md`；Stage 详情：`docs/review-phase2-fix/stages.md`。

## 1. Stage 表

| Stage | 改动项 | 编排形态 | commit message | 门禁（全量测试 agent 执行） |
|-------|--------|----------|----------------|------------------------------|
| S01 | TE-16、TE-12、TE-13 | pipeline 串行 A→B | `fix: cargo fmt 基线修复 + knip 零误报配置（TE-16/12/13）` | fmtCheck / clippy / tsCheck / eslint / rustTest / frontendTest / knip |
| S02 | TE-06、TE-07、TE-14 | pipeline 串行 A→B→C | `fix(deps): dialog 2.7.2 + typescript ^7.0.2 主字段直改 + WDIO dedupe（TE-06/07/14）` | tsCheck / eslint / frontendTest / e2eBuild |
| S03 | SEC-15、SEC-17、BE-22、BE-24、BE-25 | 并行 4 | `fix(security): shell 白名单 fallback 收窄 + watcher 校验异步化/大小写 + 锁中毒可观测 + user 层审计（SEC-15/17、BE-22/24/25）` | clippy / fmtCheck / rustTest |
| S04 | SEC-16 | 单 agent（豁免：P1 强耦合单项） | `fix(state): set_project_root tokio::Mutex 串行化（SEC-16）` | clippy / fmtCheck / rustTest |
| S05 | BE-10、FE-38 | 单 agent（同 effect 两处） | `fix(workspace): 空页面 stopWatch + effect await 串行（BE-10、FE-38）` | tsCheck / eslint / frontendTest / e2eBuild |
| S06 | FE-37、FE-36、BE-23 | pipeline 串行 A→B | `refactor(stores): switchToPage IPC 上提 + MAX_PAGES 全局化 + 切换失败 toast（FE-37/36、BE-23）` | tsCheck / eslint / frontendTest / e2eBuild |
| S07 | SEC-04、FE-08、FE-10、FE-42、FE-43、FE-44、FE-45 | 并行 3 | `fix(frontend): 静默 catch 可观测化 + getErrorMessage 统一 + nonce 威胁模型守卫（SEC-04、FE-08/10/42/43/44/45）` | tsCheck / eslint / frontendTest / l3Test |
| S08 | FE-39、FE-40、FE-41 | 并行 2 | `fix(explorer): 选中滚动跟随 + 已删目录行清理（FE-40/41；FE-39 验证已固化）` | tsCheck / eslint / frontendTest |
| S09 | FE-35、FE-46、FE-47、FE-48 | 并行 2 | `fix(stability): 死代码清除 + ErrorBoundary 重试 + 关窗总超时 + waitFor abort 清理（FE-35/46/47/48）` | tsCheck / eslint / frontendTest / knip / e2eBuild |
| S10 | FE-31、DOC-11、DOC-12、DOC-13、DOC-14、TE-15 | 并行 3 | `docs: editor CLAUDE.md 新建 + 用例数校正 + Phase 2 决策/债务登记（FE-31、DOC-11~14、TE-15）` | 全门禁终跑：fmtCheck / clippy / tsCheck / eslint / rustTest / frontendTest / l3Test / knip / e2eBuild |

## 2. 门禁命令（键名 → 命令；config.json commands 之外的本批新增项单列）

| 键名 | 命令 |
|------|------|
| tsCheck | `npx tsc --noEmit` |
| eslint | `npx eslint src/` |
| clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` |
| fmtCheck（本批新增） | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` |
| frontendTest | `npm test` |
| rustTest | `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` |
| l3Test | `npm run test:l3` |
| e2eBuild | `npx tauri build --debug --no-bundle` |
| knip（本批新增） | `npx knip --production` |

> npm ls 版本断言（S02）不进全量测试命令——verify agent 以静态命令实跑取数（断言口径写入 verify/stage-02.md）。

## 3. git add 路径枚举（每 Stage commit 限定）

```
src/  src-tauri/  e2e-tests/  test/
.claude/CLAUDE.md  .claude/test-inventory.md  .claude/adr.md
docs/
package.json  package-lock.json  knip.json  .github/workflows/ci.yml
```

> 照 config.json `workflow.gitAddPaths`，另补本批触碰的根配置文件四件（package.json/package-lock.json/knip.json/ci.yml）——S01/S02/S09 需要。

## 4. fix-loop args 规范

verify 不通过时主 agent 调用 `docs/review-phase2-fix/workflows/fix-loop.js`：

```js
Workflow({
  scriptPath: "docs/review-phase2-fix/workflows/fix-loop.js",
  args: {
    stage: NN,                       // Stage 编号（数字）
    failedItems: [...],              // verify 返回的未通过项 ID
    fixContext: "...",               // verify details 证据原文
    verifyFile: "docs/review-phase2-fix/workflows/verify/stage-NN.md",
    constraints: "",                 // 本批各 Stage 无特殊纪律（可选传空）
    testCommands: "...",             // 可选：覆盖默认门禁；缺省 = 核心六项
                                     // （tsCheck/eslint/clippy/fmtCheck/frontendTest/rustTest，
                                     //  不含 e2eBuild/knip/l3——Stage 特有门禁由 verify agent
                                     //  按 verify 文件静态核实）
  },
})
```

最多 3 轮（config.json `workflow.fixMaxRetries`）。

## 5. 进度跟踪表

| Stage | 状态 | commit hash | verify 结果 | 人工验证点 |
|-------|------|-------------|-------------|------------|
| S01 | ✅ | ee02b4a | TE-16/13 fixed；TE-12 fix-loop 1 轮（真死代码删除+注释背书+执行报告） | — |
| S02 | ✅ | 1233336 | TE-06/14 fixed；TE-07 fix-loop 1 轮（妥协正式化）。妥协结论（TE-07）：typescript-eslint 8.x 全系不支持 TS7——8.67.0 最新版 peerDependencies 上限 `<6.1.0` 且模块加载期硬拒 TS7，D14 fallback 三支（升级/overrides/暂停 type-aware）全部实测走尽；保留 side-by-side 形态（typescript6 包装器 + @typescript/native 7.0.2 供 tsc bin + typescript-eslint 8.67.0），全门禁绿。升级触发条件：typescript-eslint issue #10940 闭环且 TS7.1 稳定发布后移除 TS6 包装器、主 typescript 直改 ^7.x（详见 s02-execution-report.md）；ADR 登记由 S10-C 收口 | 真实产物冒烟 |
| S03 | ✅ | 223ac89 | SEC-17/BE-22/BE-24 fixed；SEC-15/BE-25 fix-loop 1 轮（新增代码 fmt 门禁修复） | 三 shell spawn（Win11+Win10） |
| S04 | ✅ | 7664caf | SEC-16 fixed（fix-loop 1 轮：新增用例 path_b clone 编译修复） | 连切沙箱不串 |
| S05 | ⬜ | | | 空页面 watcher 停止 |
| S06 | ⬜ | | | 切换无回归 + toast 实测 |
| S07 | ⬜ | | | — |
| S08 | ⬜ | | | — |
| S09 | ⬜ | | | 多 session 关窗时长 |
| S10 | ⬜ | | | — |

> 状态：⬜ 未开始 / 🔄 进行中 / ✅ 完成。每 Stage commit 前跑全门禁（TE-16 根因留痕）。
