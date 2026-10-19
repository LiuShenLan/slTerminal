# 主题库

> 版本化主题文档、自定义主题文件与可分享主题包的有界检查；运行时值权威在 `slterm_settings`。

## 决策与红线

- **职责分层**：`document` 拥有版本化 JSON 信封，`store` 拥有自定义主题文件，`package` 拥有可分享 ZIP 的有界检查；运行时定义值的唯一权威仍是 `slterm_settings`。原因：格式与持久化不应复制值模型。
- **主题文档**：版本化 JSON 信封，字段以 `deny_unknown_fields` 校验；内置主题由同一文档形态生成。原因：导入/导出必须走同一格式。
- **主题包有界检查**：包文件上限 48 MiB、解包后总字节上限 64 MiB、单个视频资源上限 32 MiB、ZIP 条目上限 64、Manifest 上限 64 KiB；整包检查先于通用 ZIP 解析器分配元数据。原因：不可信主题包不得拖垮进程。
- **Manifest 与主题文件固定名**：`manifest.json` + `theme.slterm-theme.json`；Manifest 版本化并拒绝未知字段。原因：包格式是外部契约。
- **冷路径同步**：主题包检查/导出是冷路径同步操作，不落渲染回调。原因：导入导出不得阻塞前台绘制。
- **Revision 前置条件**：自定义主题写入带 revision 前置条件，过期写入被拒绝。原因：并发编辑不得静默覆盖。
- **校验门**：主题色值须过语义槽 WCAG 校验；不达标的主题不得入目录。原因：仅暗色与可读性是产品约束。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：文档信封往返与未知字段拒绝、包上限与条目上限、Manifest 版本、revision 前置条件、导出/导入往返、WCAG 校验门。
- 主题包导入/导出的 UI 接线由构建产物人工验收，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增主题：以主题文档形态加入内置目录，并通过 WCAG 校验。
- 修改包格式：同步 Manifest 版本、检查逻辑与导入/导出用例。
- 自定义主题写入：必须带 revision 前置条件，冲突时拒绝而非覆盖。
