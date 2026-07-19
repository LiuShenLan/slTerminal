# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **CV-TE-01**：`e2e-tests/test.e2e.ts` 存在 describe（commit 视图相关命名）含 2 条用例（Read 确认）：① 列表渲染——`commit-changes` 下含 modified 文件名、`commit-unversioned` 下含 untracked 文件名；② 双击 modified 项后出现 title 含 `(git diff)` 的面板且 `[data-e2e="diff-left"]` 与 `[data-e2e="diff-right"]` 存在
- **CV-TE-01 选择器纪律**：新增用例中选择器全部 `data-e2e` 属性（grep 新增 describe 段内无 `style=` / CSS 类名选择器）
- **CV-TE-02**：`e2e-tests/gitScaffold.ts`（或等价 util）存在；导出建仓库函数（git init + config + commit 基线 + 场景产出，经 `child_process`）与清理函数；用例 after 钩子调用清理（Read 确认）；不同用例 tempdir 隔离
- **CV-TE-02 契约**：git 操作只在 Node spec 侧（grep `execSync` 不出现在经 `browser.execute` 注入的页面脚本字符串中）
- **全量**：`npm run e2e` 通过（含 build:e2e 构建成功 + wdio 全绿——含既有 14 条用例无回归）

## 全量测试（全部通过为门禁）

1. `npm test`
2. `npm run e2e`（= build:e2e + wdio；e2e-tests 不在根 tsconfig include，构建级门禁由 build:e2e 承担）
