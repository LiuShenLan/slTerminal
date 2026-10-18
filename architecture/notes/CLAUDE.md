# 因果 note 规则

## 目的

- `architecture/notes/` 记录非平凡决策**为什么**存在：上下文、被否决方案、证据、后果与替代条件。
- 与 `.claude/adr.md` 分工：adr 收「当前生效决策的当前形态」（原位收敛、需快速索引）；notes 收完整成因档案与被推翻决策的存档（不可改写、随时间增长）。
- 计划、任务清单与临时工作记录不入此目录。

## 何时写

依赖方向、核心 ownership、持久化格式、协议、线程/生命周期、重要性能合同、治理变更，或易被遗忘的跨层事故。样式、普通单文件修复、机械重构、常规依赖升级不写。

## 位置与历史

- 路径镜像 owning code path，例如 `architecture/notes/slterm_terminal/input/2026-09-19-example.md`。
- 文件名 `YYYY-MM-DD-kebab-case.md`；一决定/一事故一文件。
- 不建全局 `INDEX.md`；由最近模块 `CLAUDE.md` 指路。
- 接受后不改写结论。演进 = 新 note 标 `Supersedes` 互链；旧 note 只加 `Superseded by` 短指针。
- notes 是仓库评审内容，只含供评审的成因与证据。
- adr 条目沿革行可写「成因档案见 `architecture/notes/<path>`」；note 的 Context 段可写「当前生效形态见 `.claude/adr.md` ADR-NNNN」。

## 必备结构

每篇决策 note ≤200 物理行，含九段：

1. `Status`
2. `Context`
3. `Evidence`
4. `Decision`
5. `Rejected alternatives`
6. `Consequences`
7. `Validation`
8. `Supersedes`
9. `Revisit when`

不存在的关系写 `None`。链接权威代码、测试或事实文档，不手抄 API 清单。测试保护当前行为；note 保留原因与被丢弃的路径。
