# tree-sitter-dockerfile（第三方锁定快照）

> Dockerfile 已生成 C grammar、scanner 与单一运行时 Rust 绑定。

## 决策与红线

- **补丁边界**：上游快照锚为 `868e44ce378deb68aac902a9db68ff82d2299dd0`，`src/` 与 `queries/` 不得偏离该快照；补丁只把旧 Rust binding 改为 `LanguageFn`。原因：grammar 审计必须与其他语言共用 tree-sitter 0.27.1，不能链入第二套原生运行时。边界：grammar 重生成、query 变更和上游升级须独立审计，不得借绑定补丁夹带。
- **许可保留**：MIT 版权与许可正文必须随快照保留；缺失许可或提交锚即阻塞。

## 验收与测试

- 经 `slterm_document` 的 `grammar_audit` 在 debug 与 release 验证 Windows x64 构建、ABI、随包 highlights 与最小样例。
