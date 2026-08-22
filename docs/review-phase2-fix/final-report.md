# Phase 2 修复执行收尾报告（final-report）

> 执行时间：2026-08-22。输入：`docs/review-phase2/`（10 维度 review，37 项修复清单）。
> 编排：`execution-plan.md` / `stages.md` / `checklist.md` + `workflows/` 脚本。
> 状态：**10/10 Stage 完成，E2E 9/9 全绿，全门禁通过**。

## 1. Stage commit 列表

| Stage | 改动项 | commit | verify 结果 |
|-------|--------|--------|-------------|
| S01 | TE-16/12/13（fmt 基线 + knip 零误报） | `ee02b4a` | TE-16/13 fixed；TE-12 fix-loop 1 轮（真死代码删除 + 51 条目背书报告） |
| S02 | TE-06/07/14（依赖升级） | `1233336` | TE-06/14 fixed；TE-07 fix-loop 1 轮（typescript-eslint 8.x 不支持 TS7，妥协正式化，s02-execution-report.md） |
| S03 | SEC-15/17、BE-22/24/25（后端安全） | `223ac89` | SEC-17/BE-22/24 fixed；SEC-15/BE-25 fix-loop 1 轮（新增代码 fmt 门禁） |
| S04 | SEC-16（root 竞态 Mutex） | `7664caf` | SEC-16 fixed（fix-loop 1 轮：并发用例 path_b clone 编译修复） |
| S05 | BE-10、FE-38（watcher 生命周期） | `be96a77` | 一次通过 |
| S06 | FE-37/36、BE-23（store 纯状态） | `34c3bf1` | 一次通过 |
| S07 | SEC-04、FE-08/10/42~45（错误处理 + nonce） | `33d6723` | 一次通过 |
| S08 | FE-39 验证、FE-40/41（explorer） | `21ba6b7` | 一次通过 |
| S09 | FE-35/46/47/48（稳定性） | `339181e` | FE-35/47/48 fixed；FE-46 fix-loop 1 轮（测试桩适配 React 19 自动恢复语义） |
| S10 | FE-31、DOC-11~14、TE-15（文档同步） | `86130ca` | 一次通过；全门禁 9/9 终跑全绿 |

**执行期追加 commit**：

| commit | 内容 |
|--------|------|
| `b25ae47` | 进度跟踪表登记 S01~S10 完成状态 |
| `77576d7` | **E2E 回归修复**：单 session 跨 spec 项目累积致 FE-36 页数上限拒绝 addPage（H6/E2E-04）——见第 4 节 |

## 2. 最终测试用例数（以实跑为准）

| 层级 | 用例数 | 备注 |
|------|--------|------|
| L1（cargo test） | 724 | 含 SEC-15 单侧拒绝、SEC-16 并发串行化新增 |
| L2（npm test） | 2627（154 文件） | S07 +6、S08 +2、S09 +1、S06 +1 等新增 |
| L3（test:l3） | 138（7 文件） | 无改动 |
| L4（E2E） | 9/9 全绿 | 经 `npm run e2e`（run-wdio.cjs 注入环境变量） |

静态门禁：tsc / eslint / clippy / fmt / knip 全绿。

## 3. 未修复项与妥协

| 项 | 结论 |
|----|------|
| TE-07（TS7 主字段直改） | **妥协**：typescript-eslint 8.x 全系 peer `<6.1.0` 且加载期硬拒 TS7，D14 三支 fallback 实测走尽；保留 side-by-side（TS6 包装器 + @typescript/native 7.0.2 供 tsc bin + typescript-eslint 8.67.0）。升级触发条件 = typescript-eslint issue #10940 闭环且 TS7.1 稳定发布（ADR-0010 登记，S10-C） |
| FE-39 | 验证项：实查 `nav-tree-history.test.tsx:302-336` 已固化「最深前缀」用例，零改动 |
| editor dirty→clean / history 偶发失败 | **flaky/环境**（执行期调查结论见第 4 节）：非本轮引入（基线 a40ee09 干净环境同样失败）；最终轮 E2E 9/9 全过。已知残留：前端 fs-event 偶发丢失（后端事件链正常、前端订阅正常、emit→listen 偶发丢），建议后续专项调查 |

## 4. E2E 回归调查与修复（重要经验）

**现象**：执行期收尾全量 E2E 出现 H6「终端跨页面存活」+ E2E-04「大负载切页签往返」稳定失败（`switchToPageAndWait` 10s 超时，activePageId 未变）。

**调查过程**（13+ 轮全量对照实验排除的假说）：
1. ❌ 环境污染（msedgewebview2 孤儿进程累积）——清理后仍失败
2. ❌ cargo 增量编译产物损坏——cargo clean 后仍失败
3. ❌ localStorage/跨轮 projects.json 污染——清空后仍失败
4. ❌ 启动恢复竞态（App.tsx lastPage 恢复）——清 localStorage 后仍失败
5. ❌ 后端 watcher 事件链——L1 真实目录集成测试过 + Rust 侧文件标记证实事件到达 emit 前

**最终根因**（文件标记 + 前端 window 标记双重实证）：
- **wdio 单 session 共享 app 实例**（wdio.conf.ts 文件头注释），前序 8 个 spec 的 `createProject` 项目在 store 累积（**24+ 项目/26+ 页**，Rust 侧 `load_projects` 恢复 + spec 内累积）
- **S06 FE-36 将 `addPage` 上限改为跨项目全局计数（MAX_PAGES=20）**——terminal spec（末位）的 addPage 被拒绝
- helpers.addPage **无条件返回 pageId**（store 拒绝被吞）→ spec 切换幽灵页 → activePageId 永不生效 → 超时
- be96a77（S06 前）按项目计数（单项目 ≤2 页）不受影响——二分确认回归由 S06 引入

**修复**（commit `77576d7`，E2E 基建层，产品语义不变）：
1. `wdio.conf.ts` `beforeSuite`：每 spec 清空 store（粒度 = spec 级；**不用 beforeTest**——wdio 层在 mocha `before()` 之后执行，会清掉 `before()` 建的项目并破坏 editor 标题等依赖 spec 内累积的用例——实证）
2. `helpers.__slterm_e2e_resetProjects`：清 projects/expandedNodes/**activePageId**（漏清 activePageId 会断链「无法获取活跃页面信息」——实证）
3. `helpers.__slterm_e2e_addPage` 拒绝返回 `null` + `specUtils.addPage` 提前抛错（拒绝可观测，防隐性超时）
4. `src/stores/projects.ts` `addPage` 返回 `boolean`（拒绝可观测）
5. `App.tsx` E2E 构建跳过 `loadAllProjects`（内联 `import.meta.env.VITE_E2E === "1"` 门控——引用 E2E_ENABLED 常量会使 helpers chunk 残留生产 dist，CI 守卫 fail，见 main.tsx:77-81）

**副产品教训**：
- E2E 必须经 `npm run e2e`/`run-wdio.cjs` 启动（`SLTERM_CLAUDE_PROJECTS_DIR` 注入，history spec 的 AQ-4 守卫会拒绝裸 `npx wdio`）
- 直接 `npx wdio` 跑多轮会在 `target/debug` 残留 msedgewebview2 孤儿进程（每轮 E2E-12 强杀 app 的副作用）
- 诊断方法：Rust 侧文件标记（stderr 不被 tauri-service 转发）+ 前端 window 标记（console 不转发）——两类诊断均须用完移除

## 5. 人工验证点（6 项，需人工实测）

计划标注的 6 项人工验证点无法自动化（GUI 交互/双机/真实 claude），**待用户实测**：

| # | Stage | 验证点 | 操作 |
|---|-------|--------|------|
| 1 | S02 | TS7 后真实产物冒烟 | 启动 `src-tauri/target/debug/slterminal.exe`，开终端/编辑器/hooks 面板各一次 |
| 2 | S03 | SEC-15 收窄后真实 claude spawn 三 shell | Win11 本机 + Win10 另一台，pwsh/PowerShell/cmd 三 shell 均能启动 claude（重点：Store 版 pwsh alias 场景） |
| 3 | S04 | A→B 快速连切页面沙箱不串 | 快速连切两项目页面，旧项目文件操作应被沙箱拒绝（root 不串） |
| 4 | S05 | 空页面 watcher 停止 | 删末页/移除活跃项目后，旧目录改动不再触发 fs-event |
| 5 | S06 | 页面切换无回归 + toast | 页面切换后文件树/终端 cwd 正常；构造 setProjectRoot 失败场景 toast 实测 |
| 6 | S09 | 多 session 关窗时长有界 | 多 session 场景关窗实测，总时长不随 session 数线性增长 |

## 6. 归档

- 本报告 + 执行产物（s01/s02-execution-report.md、progress 表）随 docs/ 已提交
- 建议 `npx tauri build --debug --no-bundle` 构建产物部署本机/win10 另一台进行人工验证
