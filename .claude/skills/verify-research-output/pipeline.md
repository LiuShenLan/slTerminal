# 验证 Pipeline

## 概述

4 阶段线性流程：并行分散→集中裁决→逐条判定→精确修改。每个阶段有明确的输入/输出/步骤。

**输入契约**：一组带来源引用的 markdown 文件 + 可选方向分组。
- 有分组（如 `structured-web-research` 产出的 D1/D2 方向）→ 每组 1 个子代理
- 无分组 → 按文件划分子代理，每文件 1 个

```
阶段 1                 阶段 2                阶段 3                阶段 4
并行验证              交叉裁决              逐条核实              修改源文件
─────────            ─────────             ─────────             ─────────
子代理 A → reviewA ↘                       每条声称 ──→ 接受? → 修改源文件
子代理 B → reviewB → 主代理提取 claims ──→             拒绝? → 不修改
子代理 C → reviewC ↗  脚本检测冲突 → 裁决   部分接受? → 仅修正确部分
                      输出 ADJUDICATION.md  无法验证? → 标记待定
```

---

## 阶段 1: 并行验证

**目标**: 对每组文件，独立验证其全部声称。

**输入**: 带引用 markdown 文件集 + 可选方向分组

**输出**: `review/{组名}/` 下的 review 文件——每个源文件对应一个 review 文件

**步骤**:

1. 确定分组：有方向分组按分组；无分组按文件。组数 N → spawn N 个子代理（>5 则分批，每批 ≤5）
2. 子代理内部流程:
   - 读取源文件，提取所有带来源引用的声称
   - 对每条声称：WebFetch 来源 URL → WebSearch 确认事实 → 交叉比对同组其他文件
   - 用 `review-template.mjs` 生成 review 文件头部：`node <skill目录>/scripts/review-template.mjs <源文件路径> <组名>`
   - 按 [output-spec.md](output-spec.md) 的错误条目模板逐条追加
   - **只输出发现的错误**，不输出正确信息
3. 子代理必须遵守的约束:
   - 每条声称的验证结论必须有反证来源（URL + 引用）
   - 不能验证的声称标注"无法验证"和原因
   - 同时做内部交叉比对——同组文件之间是否有矛盾
   - 写入前对照 [lessons/error-taxonomy.md](lessons/error-taxonomy.md) 的 4 种错误类型判定

**子代理 prompt 核心要素**:
```
你的任务是验证以下检索文件。
- 源文件: [列出该组所有文件路径]
- 输出: 每个源文件一个 review 文件，头部用 node <skill目录>/scripts/review-template.mjs 生成
- 方法: WebSearch + WebFetch 外部验证 + 内部交叉比对
- 格式: 严格按 output-spec.md 错误条目模板
- 规则: 只输出不正确的信息；错误类型按 lessons/error-taxonomy.md 判定
- 你不得 spawn 任何子代理
```

**确定性约束**:
- 子代理数量 = 组数（无分组则 = 文件数；>5 分批，每批 ≤5）
- 每个 review 文件与原文件 1:1 对应

---

## 阶段 2: 交叉裁决

**目标**: 识别不同文件/方向之间的矛盾，查权威来源裁决。

**输入**: 全部 review 文件 + 全部源文件

**输出**: `review/ADJUDICATION.md` — 裁决文件

**步骤**:

1. **AI 提取 claims**：通读全部源文件，对同一事实点（版本号、数量、布尔支持性、优先级顺序等）提取声称，赋予相同 `field` 名，写成 claims JSON：
   ```json
   [{"direction":"D1","file":"a.md","line":12,"field":"事件总数","value":"30 个事件","source":"https://..."}]
   ```
2. **确定性检测**：`cat claims.json | node <skill目录>/scripts/detect-conflicts.mjs` → 冲突矩阵（boolean_inversion / count_discrepancy / value_mismatch）
3. **AI 裁决**：对每个冲突，按 [lessons/verification-methods.md](lessons/verification-methods.md) 的证据层级查证:
   - 第 1 优先: 官方文档/API 响应
   - 第 2 优先: 源代码/结构化数据端点
   - 第 3 优先: 社区权威来源
4. 裁决结果写入 ADJUDICATION.md（格式见 [output-spec.md](output-spec.md)），标注严重程度(P0/P1/P2)

**确定性约束**:
- 冲突检测只经 `detect-conflicts.mjs`，不凭记忆比对
- 裁决优先级严格按证据层级

---

## 阶段 3: 逐条核实（全量复核）

**目标**: 对阶段 1 输出的每个 review 文件中的**每条**错误声称，独立复核后判定接受/拒绝/部分接受。

**输入**: 全部 review 文件 + ADJUDICATION.md + 源文件

**输出**: verify 文件（`review/verify/{组名}-verify.md`）——每条声称的判定结论

**步骤**:

1. 对每个 review 文件的每条声称（**全量，不抽查**）:
   - 独立验证: WebSearch/WebFetch 确认 review 声称是否成立
   - 对照 ADJUDICATION 裁决
   - 判定反证 kind（url_inaccessible / url_content_mismatch / official_doc_differs / other_file_contradicts / self_contradiction / outdated），调 `classify-error.mjs` 得错误类型+严重程度：
     ```bash
     echo '{"kind":"official_doc_differs","correctInfo":"...","citedSource":"..."}' | node <skill目录>/scripts/classify-error.mjs
     ```
2. 每条声称输出一个判定:
   - **正确** → 记录修改行动
   - **部分正确** → 记录仅修改正确部分
   - **不正确** → 记录拒绝原因（review 本身有误）
   - **无法验证** → 记录原因，标记待第二轮
3. 写入 verify 文件（格式见 [output-spec.md](output-spec.md) 汇总表模板）
4. 判定失误模式对照 [lessons/reviewer-pitfalls.md](lessons/reviewer-pitfalls.md)

**核心原则**: 默认不修改——只有能证实 review 正确时才修改源文件。不确定时不行动。

---

## 阶段 4: 修改源文件

**目标**: 对阶段 3 判定为"正确"和"部分正确"的声称，精确修改源文件。

**输入**: verify 文件的修改清单 + 源文件

**输出**: 修改后的源文件

**步骤**:

1. 按 verify 文件的"行动"列逐条修改
2. 修改规则严格按 [lessons/edit-discipline.md](lessons/edit-discipline.md):
   - 用 Edit 不用 Write；修改前后各看 3 行上下文
   - 改完一处立即验证上下文不破坏（表格连续、标题归属、列表层级）
   - 全局搜索修改涉及的关键词，确认无内部矛盾
3. 记录修改清单

**确定性约束**:
- 不修改的条目绝不碰源文件
- 修改范围 ≤ verify 文件标记的行号范围
