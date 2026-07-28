# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；门禁命令任一失败则相关项判 not_fixed。
> 行号引用为修复前快照（checklist 实证 2026-07-28），修复后可能漂移——以用例名/符号定位为准。

## 断言清单

- **V1（PF2-TE-09 用例 2a）**：`e2e-tests/test.e2e.ts` 用例 2a 断言反转——创建终端后 `agent-status-row` **不出现**（纯 shell 无行）；不存在「初始扫描生成 🟡 行」旧断言（语义式：Read 用例 2a 确认无"创建终端即建行"的等待/断言逻辑，不限写法）。
- **V2（PF2-TE-09 用例 2b）**：用例 2b 无「等待静态行出现」前置步骤（原 :1777-1782 已删）；首个信号文件到达后行出现且含 ⚡；Stop→✅、SessionEnd→行消失断言保留（Read 用例 2b 逐步确认）。
- **V3（PF2-TE-09 新增 3 条）**：新增 3 条常驻用例存在（语义式：切项目用量保持 / SessionEnd 删行后切项目不复活 / 会话终端关页签删行，用例名不限——Read Agent Status describe 逐条点名）。
- **V4（PF2-TE-09 R2 变体）**：R2 变体使用真实 transcript JSONL 文件（Node 端写盘，合法 JSONL 且含 `message.usage` 四字段行——`input_tokens`/`output_tokens`/`cache_read_input_tokens`/`cache_creation_input_tokens`）+ 信号文件 transcriptPath 指向该文件（非 mock contextUsage）——Read 用例源码确认写盘与信号文件两要素。
- **V5（PF2-TE-09 R4 变体）**：R4 变体关页签用 `__dockviewApi.removePanel`（grep 命中）；无 `panel.close()` 调用（grep 零命中——R4 原始探针教训：`panel?.close is not a function`）。
- **V6（门禁）**：`npm run build:e2e` 成功（exit 0）+ `npm run wdio` 全量绿（含新增 3 条与反转后的 2a/2b）——以测试 agent 实跑结果为准。

## 全量测试（全部通过为门禁）

1. `npm run build:e2e`（= `cross-env VITE_E2E=1 tauri build --debug --no-bundle`——必须 VITE_E2E=1，否则 helper 被 tree-shake、wdio 全卡 Workspace 未就绪）
2. `npm run wdio`

> 门禁特殊性：`e2e-tests/test.e2e.ts` 不在根 tsconfig include 内（tsc/eslint 不覆盖）——本 Stage 以构建级 + 行为级实跑为门禁，无静态检查属正常。
