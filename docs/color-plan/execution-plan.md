# 配色系统重构 执行编排参数（execution-plan）

> 本文件只写任务特定编排参数；通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作细则）单一真值源在 `/systematic-changes-execute`，不复制。
> 清单真值源：`docs/color-plan/checklist.md`；Stage 划分与契约：`docs/color-plan/stages.md`。

## Stage 编排总表

| Stage | 脚本 | verify | 项 | 门禁命令 | commit |
|-------|------|--------|----|---------|--------|
| 01 方案骨架 | `workflows/stage-01-schemes-skeleton.js` | `workflows/verify/stage-01.md` | SCH-01~05 | tsc + eslint | `refactor(theme): 配色方案系统骨架——schemes/ + SchemeRegistry + overrides` |
| 02 facade 切换 | `workflows/stage-02-colors-facade.js` | `workflows/verify/stage-02.md` | FAC-01/02、TST-01 | tsc + eslint + `npm test` | `refactor(theme): colors.ts facade 化——31 导出代理 active 方案 + 死配置清理` |
| 03 消费点迁移 | `workflows/stage-03-consumers.js` | `workflows/verify/stage-03.md` | FAC-03、CON-01~06 | tsc + eslint + `npm test` + `npm run test:l3` | `refactor(theme): 消费点迁移——oneDark 四处/dockview/allotment/终端 adapter/JsonMode 违规收敛` |
| 04 启动序列 | `workflows/stage-04-bootstrap.js` | `workflows/verify/stage-04.md` | BOOT-01~03 | tsc + eslint + `npm test` + `npx vite build` | `refactor(theme): 启动序列——main.tsx 动态 import 链 + App.css 归位` |
| 05 测试补全 | `workflows/stage-05-tests.js` | `workflows/verify/stage-05.md` | TST-02~05 | `npm test` | `test(theme): scheme-registry/overrides 测试新增 + test-inventory 同步` |
| 06 文档同步 | `workflows/stage-06-docs.js` | `workflows/verify/stage-06.md` | DOC-01~06 | 无编译门禁——`git diff --name-only HEAD` 供 verify 断言取数 + verify grep 断言承担 | `docs(theme): 配色方案文档同步——CONTEXT/ADR/CLAUDE.md/color 两文档` |
| 07 验收 | `workflows/stage-07-acceptance.js` | `workflows/verify/stage-07.md` | ACC-01~06 | spec §10 六项（含 `npm run build:e2e` + `npm run wdio`） | 无 commit |

所有脚本经 `Workflow({ scriptPath: "docs/color-plan/workflows/<file>" })` 调用；verify 文件为 stage 脚本与 fix-loop 共用标尺。

## git add 路径枚举（Stage 级精确路径）

| Stage | git add 路径 |
|-------|-------------|
| 01 | `src/theme/schemes/ src/theme/schemeRegistry.ts src/theme/overrides.ts docs/color-plan/` |
| 02 | `src/theme/colors.ts src/theme/index.ts src/__tests__/colors.test.ts` |
| 03 | `src/panels/editor/useCodeMirror.ts src/panels/gitshow/GitShowPanel.tsx src/panels/diff/DiffPanel.tsx src/panels/hooksConfig/JsonMode.tsx src/workspace/PageDockviewHost.tsx src/workspace/Workspace.tsx src/panels/terminal/theme.ts` |
| 04 | `src/main.tsx src/App.tsx src/App.css src/__tests__/bootstrap.test.ts` |
| 05 | `src/__tests__/scheme-registry.test.ts src/__tests__/overrides.test.ts .claude/test-inventory.md`（+ TST-04 失效才触的四个测试文件，触则如实追加） |
| 06 | `CONTEXT.md .claude/adr.md .claude/CLAUDE.md src/theme/CLAUDE.md src/panels/CLAUDE.md docs/color-implementation.md docs/color-inventory.md` |
| 07 | 无 commit |

> 偏离说明：`CONTEXT.md` 与 `.claude/adr.md` 不在 config.json `gitAddPaths` 白名单内，为 Stage 06 正当扩展（checklist 修正记录 3）。

## fix-loop args 规范

调用：`Workflow({ scriptPath: "docs/color-plan/workflows/fix-loop.js", args: {...} })`，args 四字段：

| 字段 | 取值规范 |
|------|---------|
| `stage` | Stage 编号（1–7） |
| `failedItems` | verify 返回的失败项 ID 数组（**不得为空**，空则不调用 fix-loop） |
| `fixContext` | verify agent 的 details（失败原因摘要），原样透传 |
| `verifyFile` | 与该 Stage 同一 verify 文件路径（`docs/color-plan/workflows/verify/stage-NN.md`）——修复循环与初验同一标尺 |
| `constraints` | 可选。仅 Stage 05/06 有值，其余 Stage 不传；值唯一真值源 = 对应 Stage 脚本头注释的 fix-loop 调用约定，此处不复制 |

重试上限：`fixMaxRetries: 3`（config.json）；超限上报人工。

## 进度跟踪表（执行期逐格填写）

| Stage | 状态 | verify 结果 | fix-loop 轮数 | commit hash | 备注 |
|-------|------|------------|--------------|-------------|------|
| 01 | ✅ | allFixed=true（15 项） | 0 | 1bfd1c8 | |
| 02 | ☐ | — | 0 | — | |
| 03 | ☐ | — | 0 | — | |
| 04 | ☐ | — | 0 | — | 人工验证点 ×2 |
| 05 | ☐ | — | 0 | — | |
| 06 | ☐ | — | 0 | — | |
| 07 | ☐ | — | 0 | — | 人工验证点 ×3，无 commit |

## 人工验证点（执行到对应 Stage 时向用户出示）

- **Stage 04**：① E2E helpers 时序 = helpers 注入在 setActive 之后；② CSS 加载顺序 = dockview.css 先、App.css 后（构建产物抽查）。
- **Stage 07**：ACC-03 零视觉截图对比 / ACC-04 降级冒烟 / ACC-05 五通道切换冒烟——逐条签字后才算 Stage 07 完成。
