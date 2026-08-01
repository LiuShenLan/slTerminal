# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-01**：`src-tauri/src/lib.rs` grep `pub mod claude_history;` 命中；`generate_handler!` 内 grep `claude_history_scan` 命中。`src-tauri/src/claude_history/mod.rs` 存在且含 `HistorySession` 结构体，字段为 `session_id/cwd/title/title_source/first_prompt/mtime_ms/cwd_exists` 七字段（snake_case），并有 serde camelCase 序列化配置（`rename_all = "camelCase"` 或逐字段 rename）；`TitleSource` 枚举含 `CustomTitle/AiTitle/Summary/FirstPrompt/None` 五变体且序列化为 `"customTitle"/"aiTitle"/"summary"/"firstPrompt"/"none"`（Read 确认 serde 属性）。
- **BE-02**：`src-tauri/src/claude_history/scan.rs` 存在；`claude_history_scan` 为 `async fn` 且阻塞遍历在 `spawn_blocking` 内（Read 确认）；排除逻辑同时覆盖：文件名以 `agent-` 开头、文件名主干非 UUID（Read 确认两分支）；不递归子目录（Read 确认只读一级子目录顶层）；单文件解析失败产生降级条目而非整体 Err（Read 确认 catch/兜底分支）；扫描根不存在返回空 Vec（Read 确认）。
- **BE-03**：`src-tauri/src/claude_history/jsonl.rs` 存在；头部扫描上限常量 = 512KB（grep `512` 命中常量定义）；可见 prompt 判定含全部四个跳过规则——`isMeta` 为 true、content 为数组（非字符串）、字符串以 `<` 开头、trim 后为空（Read 逐一确认）；prompt 截断 200 字符（grep `200` 命中截断处）。
- **BE-04**：`jsonl.rs` 含尾部扫描函数，读取上限 64KB（grep `64` 命中常量）；中途起始跳首行逻辑存在（Read 确认）；逆序查找 custom-title 优先于 ai-title（Read 确认优先级顺序）；标题回退链顺序为 custom-title → ai-title → summary → 首条 prompt（Read 确认合成函数分支顺序）。
- **BE-05**：`scan.rs` 中 `mtime_ms` 取自文件 metadata modified（Read 确认）；`cwd_exists` = cwd 非 null 且路径 `is_dir()`，cwd 为 null 时 false（Read 确认）。
- **SEC-02 / BE-06**：`scan.rs` 含 `resolve_projects_root`（或同义单点函数），先读 env `SLTERM_CLAUDE_PROJECTS_DIR`（grep 命中该字符串），缺省回退 home `.claude/projects`（Read 确认）；env 读取在每次 scan 调用时执行而非进程启动缓存（Read 确认调用位置）。
- **BE-09**：`jsonl.rs`/`scan.rs` 的 `#[cfg(test)]` 模块中，以下每类至少一条测试（Read 测试函数逐一核对，不接受合并含糊）：标题回退链 5 态、prompt 跳过 4 类、EOF 截断行、未知 type 忽略、无 cwd、大文件头限+尾扫协同、扫描排除 3 类、env 覆盖（测毕恢复 env）、损坏文件降级条目、扫描根不存在空数组；测试使用 tempdir 隔离且路径经 `dunce::canonicalize`（grep 命中）。
- **DTO 双边（意图断言）**：`HistorySession` 的 serde 输出键集合恰为 `sessionId/cwd/title/titleSource/firstPrompt/mtimeMs/cwdExists` 七键（测试或 Read 确认，防字段漂移）。
- **禁区**：`git diff` 本 Stage 不含 `src-tauri/src/pty/` 下任何文件改动（grep diff 文件清单确认）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
