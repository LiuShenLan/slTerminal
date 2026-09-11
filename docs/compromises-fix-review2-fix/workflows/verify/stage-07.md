# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；本 Stage 为注释/文档修订，零生产逻辑变更。

## 断言清单

- **DOC-01**：`rg "预览域（无 CSP）|预览域无 CSP|无 CSP 下" src/__tests__/markdown-assets.test.ts e2e-tests/html.e2e.ts e2e-tests/markdown.e2e.ts` 零命中；`rg "无全局 CSP" e2e-tests/html.e2e.ts` 命中行均含 meta CSP 补充说明（Read 逐行确认）；五处替换文本与 checklist DOC-01「修复步骤」段逐字一致（Read 对照）。
- **DOC-02**：`rg "预览域（无 CSP）|预览域无 CSP|预览域维持无 CSP" .claude/adr.md .claude/test-exemptions.md` 零命中；`rg "SEC-02 起" .claude/adr.md` ≥ 3 命中（:446/:459/:505 区行内注记）+ `.claude/test-exemptions.md` 1 命中（:39 行）；Read 确认四处均为行内注记追加、ADR 段落结构未打断。
- **DOC-05**：`rg "提交即登记本 Stage 行" .claude/skills/systematic-changes-execute/SKILL.md` 1 命中；Read 确认该条位于 5.6 主 agent 编排节列表内、含「禁止跨 Stage 捎带」与实证出处。
- **TE-05**：`rg "getProjectRootPath" src/workspace/CLAUDE.md` 命中；Read :75 区确认签名与 `src/workspace/tabChrome.tsx` 第 4 参（`getProjectRootPath?: (pageId: string | null) => string | undefined`）逐字一致（对照真实代码核实，防文档撒谎）且含回调缘由半句（S11 回归修复）。

## 全量测试（全部通过为门禁；逐条串行执行，禁并行）

1. `git status --porcelain` 甄别：本 Stage 变更仅预期 7 文件（markdown-assets.test.ts / html.e2e.ts / markdown.e2e.ts / adr.md / test-exemptions.md / SKILL.md / workspace/CLAUDE.md）——零生产逻辑变更确认
2. `npx tsc --noEmit`
3. `npx eslint src/`
