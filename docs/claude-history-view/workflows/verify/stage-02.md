# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-01**：`src-tauri/src/claude_history/ops.rs` 存在；含 sessionId 校验函数（grep `validate` 或正则字面量命中），UUID 形态校验拒绝非 UUID 输入；`claude_history_delete` 与 `claude_history_rename` 均**不接受任何路径参数**（Read 两命令签名确认：仅 `session_id`、rename 另加 `new_title`）；文件定位为遍历扫描根一级子目录查找（Read 确认，复用 Stage 01 遍历逻辑或共享私有函数）。
- **BE-07**：`claude_history_delete` 实现：`remove_file` 删 `<id>.jsonl` + 同名 `<id>/` 目录存在时 `remove_dir_all`（Read 确认两分支）；jsonl 不存在返回 `AppError::Validation`（Read 确认错误分支）；阻塞 I/O 在 `spawn_blocking` 内。
- **BE-08**：`claude_history_rename` 实现：`new_title` trim 后非空且 ≤200 字符校验（Read 确认）；`OpenOptions.append` 追加写入（Read 确认，非整文件重写）；追加行由 serde_json 序列化生成且含 `"type":"custom-title"`、`"customTitle"`、`"sessionId"` 三键（Read 确认构造结构）；行尾带 `\n`。
- **lib.rs 注册**：`generate_handler!` 内 grep `claude_history_delete`、`claude_history_rename` 均命中（连同 Stage 01 的 scan 共三命令）。
- **BE-10**：`ops.rs` 的 `#[cfg(test)]` 模块中，以下每类至少一条测试（Read 逐一核对）：delete 删 jsonl+同名目录、delete 仅 jsonl 无目录、delete 不存在 → Err、rename 追加行反序列化三键断言、rename 空/空白/>200 拒绝、sessionId 非法 4 类拒绝（含 `..`、含 `/`、含 `\`、非 UUID 形态）；测试用 tempdir 隔离 + `dunce::canonicalize`（grep 命中）。
- **越界防护（语义式）**：delete/rename 的全部文件操作路径均由「扫描根 + 一级子目录名 + 校验过的 sessionId」拼接派生，不存在将前端传入字符串直接拼入文件路径的代码点（Read 两命令实现确认，不限变量名）。
- **禁区**：`git diff` 本 Stage 不含 `src-tauri/src/pty/` 下任何文件改动。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
