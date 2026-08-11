# review-fix 修复完成报告

> 执行方式：`/systematic-changes-execute` 逐 Stage 串行（Workflow + verify 断言 + fix-loop 循环）。真值源：`docs/review-fix/checklist.md`（30 条）+ `stages.md`（划分/契约）+ `execution-plan.md`（编排参数）。
> 全部 30 条 review 问题（1 P0 + 6 P1 + 11 P2 + 12 P3）**全部修复并验证**，无未修复项。

## Stage commit 列表

| Stage | commit | 内容 | fix-loop 轮次 | 验证结果 |
|-------|--------|------|--------------|---------|
| 01 | `16a767a` | 安全族（AQ-1~4/ZQ-5）+ 守卫强化（CS-1/2） | 0（主 agent 直补 test-inventory + 复跑排除 flaky + inline verify） | allFixed（L1 597 / L2 2411 / L3 / L4 9 spec） |
| 02 | `bed433d` | 前端正确性族（ZQ-1~4/6/7）——keyOf/resolvePayloadCliId 单点 + null 建行 | 0（一次通过） | allFixed（L1 597 / L2 2421 / L3） |
| 03 | `a612fc0` | transcript 中性化全链路（KZ-2/3）——usageSourcePath 更名 + SCRIPT_VERSION 3 | 1（hooks.e2e.ts 测试健壮性：rowAboveEditor 定位 bug + 面板残留） | allFixed（L4 9/9） |
| 04 | `ef0ae64` | hub 编辑器分派 + 层抽象入 profile（KZ-1/4/5） | 1（vitest.l3.config.ts 补 codemirror inline；spawn.rs 断言大小写不敏感） | allFixed（vite build 无循环；L1 597） |
| 05 | `d19869e` | mockcli 验收强化（KZ-7/CS-3）——双向分派断言 + L4 关键路径 | 1（hub 断言限定面板容器作用域；helpers.ts extractErrorText） | allFixed（L4 9/9；AC-4④ 中间态消除） |
| 06 | `495ec43` | 文档一致性修复 + 终态核对（YS-1~5/WD-1~4/KZ-6） | 1（YS-3 残留 14 处先例引用；test-inventory 计数对齐） | allFixed（全量 8 条门禁全绿） |

归档 commit（本报告 + execution-plan 进度表 + CONTEXT.md 累积更名）见收尾 commit。

## 最终测试用例数（实跑，2026-08-12 收尾终验）

| 层级 | 用例数 | 说明 |
|------|--------|------|
| L1 | 597 | `cargo test --test-threads=1`（含 AQ-2/AQ-3/ZQ-5 新增 5 条） |
| L2 | 2427 | `npm test` 139 文件（it.each 展开口径差 1 以实跑为准，test-inventory 注③ 校正） |
| L3 | 138 | `npm run test:l3` 7 文件 |
| L4 | 39 active + 2 skip | `npm run e2e` 9 spec 100%（+2 用例、2 条既定豁免） |
| **全量** | **3203** | 597 + 2427 + 138 + 41 |

## 人工验证点实测结果

| Stage | 验证点 | 结果 |
|-------|--------|------|
| 01 | ① fixture 缺失终止 | ✅ 实测通过：`fixtures/claude-projects` rename → `node e2e-tests/run-wdio.cjs` exit 1 + 文案「fixtures/claude-projects 缺失，E2E 终止——防止回落真实 ~/.claude/projects」→ 恢复 |
| 01 | ② 含单引号 cwd 恢复命令 | ✅ 实测通过：`cd 'C:\Users\...\Bob''s Project' && pwd` 在 pwsh 正确解析进入目录（`''` 转义生效） |
| 02 | null 映射事件无图标行视觉 | ⏳ 待用户实测（SessionStart 丢失模拟——侧栏出现无图标行；L2 已断言 status=null 建行，视觉需真实 app） |
| 03 | 「版本过旧」→ 重新注入 → 用量条 | ⏳ 前半已实证：用户环境 reporter 为 **SCRIPT_VERSION=2**（旧版），代码已升 v3——打开 app 显示「版本过旧」；重新注入 + 用量条更新为用户操作 |
| 04 | hub 面板 claude 编辑器分派全链 | ⏳ 待用户实测（层切换/GUI/JSON/保存/注入按钮/重启提示；L2 分派用例 + L4 mockcli 分派已自动覆盖同链） |
| 06 | 终验人工走查（真实 claude 全功能回归） | ⏳ 待用户实测（页签四态/用量条/hooks 注入三态/历史区/恢复编排/hub 面板） |

## 执行期发现的既有问题（非 30 条清单内，随行修复）

| 问题 | 根因 | 修复 |
|------|------|------|
| diff-panel.test.tsx 2 例瞬时失败（Stage 01 全量） | 并行高负载下 waitFor 超时（flaky） | 复跑 2411/2411 全绿排除（非代码缺陷） |
| hooks.e2e.ts hub 断言（Stage 03/05 L4） | ① rowAboveEditor 用容器比位恒 false（compareDocumentPosition 祖先不置位）；② 面板/终端页签 logo 残留污染全页计数（spec 间共享 app） | 编辑器参照改取编辑器槽；断言限定面板容器作用域；开前关闭 + finally 回收 |
| L3 suite 加载失败（Stage 04 引入） | profiles/claude 运行期 import 编辑器 → codemirror-json-schema ESM 无扩展名导入进 L3 链，vitest.l3.config.ts 缺 inline | 照 vitest.config.ts 先例补 `deps.inline` |
| spawn.rs 环境块断言（Stage 04 fix-loop 发现） | 大小写敏感匹配 SYSTEMROOT，Windows 键名原始大小写（SystemRoot=） | 断言统一转大写比对 |
| helpers.ts 桩错误文案（Stage 05） | `String(err)` 对 Tauri 2 serde 外部标记错误（`{validation:...}`）得 [object Object] | `extractErrorText` 递归提取首字符串字段 |
| YS-3 换名残留（Stage 06） | registry-doc 把退役先例换指同样退役的 tabRules.ts（10 处）+ 2 处代码注释 | 统一改指现存注册表/机制描述，溯源形态保留 |

## 收尾状态

- 全部 6 个 Stage commit + 归档 commit 落盘；工作区无预期外残留
- 全量门禁（静态 4 + L1/L2/L3/L4）最终复跑全绿
- 待用户操作：注入 v3 reporter（「版本过旧」提示后）与终验人工走查
