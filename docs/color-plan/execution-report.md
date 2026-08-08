# 配色系统重构 执行收尾报告

> 执行日期：2026-08-08。执行流程：`/systematic-changes-execute docs/color-plan/execution-plan.md`（7 Stage 串行 + fix 循环 + 人工验证）。

## Stage commit 列表

| Stage | commit | message |
|-------|--------|---------|
| 01 方案骨架 | `1bfd1c8` | refactor(theme): 配色方案系统骨架——schemes/ + SchemeRegistry + overrides |
| 02 facade 切换 | `e66e609` | refactor(theme): colors.ts facade 化——31 导出代理 active 方案 + 死配置清理 |
| 03 消费点迁移 | `a9863b6` | refactor(theme): 消费点迁移——oneDark 四处/dockview/allotment/终端 adapter/JsonMode 违规收敛 |
| 04 启动序列 | `ad8918d` | refactor(theme): 启动序列——main.tsx 动态 import 链 + App.css 归位 |
| 05 测试补全 | `186e21c` | test(theme): scheme-registry/overrides 测试新增 + test-inventory 同步 |
| 06 文档同步 | `19ad419` | docs(theme): 配色方案文档同步——CONTEXT/ADR/CLAUDE.md/color 两文档 |
| 07 验收 | 无 | 验收 Stage 不 commit |
| ACC-05 修复 | `311fa8d` | fix(theme): editorColorOverrides 层叠胜出——mountStyles reverse 特异性修复 + 守卫测试 |

## 最终测试用例数

| 层级 | 用例数 | 说明 |
|------|--------|------|
| L2 前端单元/集成 | **2279**（135 文件） | 含新增 scheme-registry 18 + overrides 7；colors.test.ts 85 |
| L3 终端 headless | **138**（7 文件） | 全绿 |
| L4 E2E | **8 specs 100%** | wdio 全绿（串行，无并行冲突） |
| 静态门禁 | tsc / eslint / vite build | 全绿 |

## 执行期修正记录（已回写 checklist 修正记录 4 条 + verify 标尺）

1. **Stage 02 fix-loop ×2**：App.css:5 过期注释 `--sl-bg-secondary` + src/theme/CLAUDE.md 死配置残留；断言 3/10 标尺矛盾修正（允许清单扩展 fix-loop 产物）。
2. **Stage 04 目标态修正（checklist 修正记录 4）**：main.tsx 静态 import 由「恰 3 个」改「恰 2 个 + 门控内联字面量」——rolldown 不折叠跨模块常量，引用 `E2E_ENABLED` 常量会残留 helpers chunk 生产 dist（A/B 实证）。连带：src/lib/CLAUDE.md 六站点表述改五站点。
3. **Stage 06 标尺修正**：断言 9 允许清单扩展 src/lib/CLAUDE.md + 编排文档 2 项（修正记录 4 连带）。
4. **ACC-05 根因修复（Stage 07 fix-loop 1 轮）**：`@codemirror/view` `mountStyles()` 将样式模块 `reverse()` 注入——oneDark 规则恒排最后、同特异性恒胜，`editorColorOverrides()` 的 background/searchMatch 覆盖全部为死代码（与 oneDark 同值时不暴露，改值即暴露）。修复：overrides.ts 竞争选择器升特异性（`&.cm-editor` 前缀，0,2,0/0,3,0 > oneDark 0,1,0/0,2,0），不依赖扩展数组顺序，四处消费点零改动；补「层叠胜出」守卫用例（overrides.test.ts 6→7）。4 个定位 subagent 分角度调查确认根因（面板分派 / CM6 机制 / 构建产物 / 取证设计）。

## 未修复项

无。全部 34 项（SCH/FAC/CON/BOOT/TST/DOC/ACC）达成。

## 人工验证点签字记录（用户实测）

| 验证点 | 结果 | 备注 |
|--------|------|------|
| Stage 04 ① E2E helpers 时序（setActive 之后） | ✅ | 用户实测通过（启动正常 + helpers 就绪 + 无报错） |
| Stage 04 ② CSS 加载顺序（dockview 先 App.css 后） | ✅ | 用户实测通过（构建产物锚点 + 视觉冒烟） |
| ACC-03 零视觉对比 | ✅ | 用户实测通过（七区域视觉冒烟，无异常色差；无重构前二进制，以代码层逐值搬运 + 视觉冒烟替代） |
| ACC-04 降级冒烟（colorScheme:"不存在" → 回退 darcula + warn） | ✅ | 用户实测通过 |
| ACC-05 五通道切换冒烟 | ✅ | 首测 CM6 通道未生效 → 根因定位 + 修复后复测通过（编辑器背景暗蓝 + 搜索高亮洋红） |

## 交付物清单

- 代码：`src/theme/schemes/`（types/darcula/index）+ `schemeRegistry.ts` + `overrides.ts` + `colors.ts` facade + `terminal/theme.ts` adapter + 消费点迁移（useCodeMirror/GitShowPanel/DiffPanel/JsonMode/PageDockviewHost/Workspace）+ main.tsx 启动序列
- 测试：scheme-registry.test.ts（18）+ overrides.test.ts（7，含层叠守卫）
- 文档：CONTEXT.md 4 术语、.claude/adr.md ADR-0002、根/theme/panels/lib CLAUDE.md、color-implementation.md + color-inventory.md（转长期参考 + §9.2 勘误回写）
- 本计划产物：docs/color-plan/（execution-plan 进度表 + stages + checklist + workflows + 本报告）
