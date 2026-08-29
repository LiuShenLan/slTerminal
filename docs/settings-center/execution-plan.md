# 设置中心（F11）执行编排参数

- 清单：`docs/settings-center/checklist.md`；Stage 划分：`docs/settings-center/stages.md`
- 脚本：`docs/settings-center/workflows/stage-NN-*.js` + `fix-loop.js`；断言：`docs/settings-center/workflows/verify/stage-NN.md`
- 通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作）以 `/systematic-changes-execute` 为唯一真值源，本文档不复制

## Stage 编排表

| Stage | 脚本 | verify | 形态 | 门禁命令 |
|-------|------|--------|------|---------|
| 01 | workflows/stage-01-backend-plan-balance.js | verify/stage-01.md | 单 agent | clippy + rustTest + rustfmt |
| 02 | workflows/stage-02-settings-framework.js | verify/stage-02.md | pipeline A→B | tsCheck + eslint + frontendTest |
| 03 | workflows/stage-03-hooks-migration.js | verify/stage-03.md | 3 并行 | tsCheck + eslint + frontendTest |
| 04 | workflows/stage-04-keybindings-page.js | verify/stage-04.md | 单 agent | tsCheck + eslint + frontendTest |
| 05 | workflows/stage-05-autoclose.js | verify/stage-05.md | 单 agent | tsCheck + eslint + frontendTest |
| 06 | workflows/stage-06-e2e.js | verify/stage-06.md | pipeline A→B | tsCheck + eslint + frontendTest + viteBuild + e2eTest |
| 07 | workflows/stage-07-docs.js | verify/stage-07.md | 2 并行 | fullCheck（实跑取数） |

门禁命令取值以 `.claude/skills/systematic-changes-plan/config.json` 的 `commands` 节为唯一真值源（fullCheck/tsCheck/eslint/clippy/frontendTest/rustTest/l3Test/viteBuild/e2eBuild/e2eTest）。

## commit message（逐 Stage）

1. `feat(plan_balance): 轮询间隔运行期可改 + plan_balance_set_interval 专用命令（F11）`
2. `feat(settings): 设置中心框架（注册表/壳/打开编排）+ 套餐余量频率页（F11）`
3. `feat(settings): hooks 配置迁入设置中心，hooksConfig 面板类型退役（F11）`
4. `feat(settings): 快捷键可视化配置页（F11）`
5. `feat(settings): 切项目自动关闭设置中心面板（F11）`
6. `test(e2e): 设置中心 L4 覆盖 + hooks 用例适配（F11）`
7. `docs(settings): F11 文档同步——术语/ADR-0012/模块文档/用例清单`

## git add 路径枚举（执行期读取）

`["src/", "src-tauri/", "e2e-tests/", "test/", ".claude/CLAUDE.md", ".claude/adr.md", ".claude/test-inventory.md", "CONTEXT.md", "docs/"]`

（config.json 默认 + `.claude/adr.md` + `CONTEXT.md`——本任务含 ADR-0012 与术语表改动。）

## fix-loop args 规范

```json
{
  "stage": "<stageNumber>",
  "failedItems": ["<verify 断言原文>", "..."],
  "fixContext": "<主 agent 补充的失败上下文（测试输出摘要等）>",
  "verifyFile": "docs/settings-center/workflows/verify/stage-NN.md",
  "constraints": "见脚本头注释（唯一真值源）"
}
```

`failedItems` 必填非空、`verifyFile` 必填——fix-loop.js 模板强制校验。

## 进度跟踪表（执行期逐格回填）

| Stage | 变更 | 门禁 | verify | commit |
|-------|------|------|--------|--------|
| 01 | ☑ | ☑ | ☑ | ☑（d23ceaa） |
| 02 | ☐ | ☐ | ☐ | ☐ |
| 03 | ☐ | ☐ | ☐ | ☐ |
| 04 | ☐ | ☐ | ☐ | ☐ |
| 05 | ☐ | ☐ | ☐ | ☐ |
| 06 | ☐ | ☐ | ☐ | ☐ |
| 07 | ☐ | ☐ | ☐ | ☐ |

## 项目禁区（写入各脚本 PREAMBLE）

唯一真值源 = `.claude/skills/systematic-changes-plan/config.json` 的 `workflow.forbiddenZones`（当前唯一条目：compute_conpty_flags 固定 0x7 禁改）——本文档不复制取值，各 Stage 脚本头注释/PREAMBLE 生成时已按 config 取值写入。

## 人工验证点登记（收尾一次性实测）

- Stage 03 后：配置钮全链（打开/hooks 三层编辑/注入卸载/切 CLI dirty/旧布局丢弃/× 确认）
- Stage 04 后：录制屏蔽（Ctrl+Shift+C 不复制/改绑生效/保留键拒绝）
- 频率页动态间隔：改 120 → 观察余量刷新节奏变化
