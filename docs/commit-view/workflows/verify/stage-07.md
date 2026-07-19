# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 文档类断言必须对照真实代码核实——文档描述与代码现状不一致即判 not_fixed（防文档撒谎）。

## 断言清单

- **CV-DOC-01**：`src/features/commit/CLAUDE.md` 存在，含职责/状态机/分派映射/测试模式段落；`src/panels/CLAUDE.md` 面板类型清单含 gitshow 与 diff（grep 命中），架构决策与文件表同步——且描述与 `src/panels/gitshow/`、`src/panels/diff/` 实际代码一致（抽查：只读配置、占位对齐、滚动同步描述对照代码核实）
- **CV-DOC-02**：`src/ipc/CLAUDE.md` 的 git.ts 行含 `git_file_at_head`（grep 命中）；`src-tauri/src/git/CLAUDE.md` 含 `git_file_at_head` 命令说明与 `oldPath`、`recurse_untracked_dirs`（grep 命中）；`src/features/sideViews/CLAUDE.md` 的注册视图描述为 3 条且含 commit；`src/workspace/CLAUDE.md` 的 titleManager 段落含 suffix 语义
- **CV-DOC-03**：`.claude/CLAUDE.md` 模块表含 `src/features/commit` 行（grep 命中）；panels 行描述与五面板现状一致；`.claude/test-inventory.md` 各层级用例数已更新且取数可核实——L2 对照全量测试结果中的 npm test 统计行（Test Files / Tests 数字）；L1/L3/L4 对照源文件静态计数（L1: grep `#[test]` 计数；L3/L4: grep `it(` 计数）核实，不依赖 cargo/wdio 实跑输出
- **语义断言**：所有改动文件为 *.md（`git diff --name-only` 确认零代码文件混入）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
