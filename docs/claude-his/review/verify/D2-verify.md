# D2 逐条核实

> 核实日期: 2026-08-01

## 验证结果汇总

| 类别 | 数量 |
|------|------|
| Review 正确，已修正源文件 | 4 |
| Review 部分正确，部分修正 | 0 |
| Review 不正确，未修改 | 0 |
| 无法验证 | 0 |
| 总计 | 4 |

## 逐条判定

### 错误 1（行 18）：isSidechain 零记录归因错误

- **判定**: 接受
- **依据**: agentyard 原文三处引用证实「252」为项目目录总数（性能统计），零记录观测来自单个 2.5MB transcript。
- **行动**: 修正 D2-transcript-jsonl-format.md 行 18——归因改为「单个 2.5MB transcript 样本」。

### 错误 2（行 48）：toolUseResult 虚构字段 filePath/structuredPatch

- **判定**: 接受
- **依据**: ccrider schema.md 原文对 toolUseResult 仅列 stdout/stderr/interrupted/isImage + WebFetch 与 TodoWrite 场景字段；toolpath messages.md 全文无 toolUseResult 词。
- **行动**: 删除 D2-transcript-jsonl-format.md 行 48 中 `filePath`/`structuredPatch`。

### 错误 3（行 33）：progress 条目虚构字段 toolName/toolInput

- **判定**: 接受
- **依据**: claude-code-types 源码 `ProgressEntry` 为 `{ type: 'progress'; data; parentToolUseID?; toolUseID? }`，无 toolName/toolInput。
- **行动**: 修正 D2-transcript-jsonl-format.md 行 33 表格 progress 行字段为 `data`/`toolUseID`/`parentToolUseID`。

### 错误 4（行 120）：类型增长序列归因错误

- **判定**: 接受
- **依据**: ccrider Entry Types 一节完整枚举仅 5 种（summary/user/assistant/system/file-history-snapshot），无 permission-mode/progress。
- **行动**: 修正 D2-transcript-jsonl-format.md 行 120——2.0.29 段仅列 5 种，permission-mode/progress 归入 2.1.x 段。

### ADJUDICATION 冲突 1（行 13）：编码规则表述不完整

- **判定**: 接受（D1 正确，D2 表述不完整）
- **依据**: 官方文档 L1 原文「non-alphanumeric characters replaced by `-`」；Windows 冒号也参与替换（#54066）。
- **行动**: 修正 D2-transcript-jsonl-format.md 行 13——编码规则改为「非字母数字字符（含 `/`、`_`、`:` 等）替换为 `-`」。