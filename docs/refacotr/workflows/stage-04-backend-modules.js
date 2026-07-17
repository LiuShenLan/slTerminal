// Stage 4 — 后端 fs/git/settings/notify（SEC-04、BE-03、SEC-05、BE-06、BE-15、BE-05、TE-13、TE-14）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 3 个并行 agent，文件不重叠（fs/mod.rs | git/mod.rs | settings.rs+notify/mod.rs）

export const meta = {
  name: 'stage4-backend-modules',
  description: 'Stage4: fs/git/settings/notify 安全与正确性修复',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件，不顺手改无关代码；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何情况下不得修改 ConPTY flags。
背景文档：修复要点详见 docs/refacotr/refactoring-checklist.md 对应 ID 条目（先读再动手）。
测试约束：cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1；git 测试用 tempdir 必须 dunce::canonicalize（CI 8.3 短名坑，见 git/CLAUDE.md）。`

phase('并行重构')
const refactorResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责 src-tauri/src/fs/mod.rs 的 3 项改动：

【SEC-04】fs_rename 行为缺陷（fs/mod.rs:244-250）
- 目标为已存在目录 → 返回 Err（不再 remove_dir_all 静默递归删除）
- 目标为已存在文件 → 先 remove_file 再 rename（与注释"覆盖"语义一致，修复 Windows 行为）
- 为两条路径各补单元测试

【BE-03】fs_write_file 全量读文件测行尾（fs/mod.rs:79-87）
- std::fs::read 改 File::open + 只读前 CRLF_SAMPLE_MAX_BYTES 字节样本判断 CRLF

【TE-14】fs 命令包装层补测
- 为 fs_read_file/fs_write_file/fs_read_dir/fs_rename 的命令包装补单元测试：参数透传、错误映射、sandbox 校验分支（Stage 1 已启用真沙箱，测试内需先 set_project_root 或用 cfg(test) 豁免机制——参考 state.rs 现有 15 条 sandbox 测试的模式）

改完跑：cargo test --manifest-path src-tauri/Cargo.toml fs -- --test-threads=1 全绿。
`, { label: 'fs-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src-tauri/src/git/mod.rs 的 3 项改动：

【SEC-05】git_status/git_diff 无沙箱校验（git/mod.rs:71-112/118/176）
- 两命令入口对 repo_path/file_path 调 crate::state::validate_path_within_root
- get_or_open_repo 的 Repository::discover 结果 workdir 同样校验（防上溯到父仓库泄露）

【BE-06】git 缓存 starts_with 误命中子仓库（git/mod.rs:78-90）
- 删除 get_or_open_repo 缓存命中条件中的 workdir.starts_with(&search) 分支，仅保留 search.starts_with(workdir)
- 补测试：缓存子目录仓库后，用父目录调用不命中该缓存

【TE-13】hunk 合并副本消除（git/mod.rs:1327-1389）
- 把测试内的 compute_diff_hunks 副本逻辑提取为生产纯函数（pub(crate)），生产代码与测试共用；删除测试副本

改完跑：cargo test --manifest-path src-tauri/Cargo.toml git -- --test-threads=1（61+ 条全绿）。
`, { label: 'git-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src-tauri/src/settings.rs 与 src-tauri/src/notify/mod.rs 的 3 项改动：

【BE-05】save_settings 的 create_dir_all 在 spawn_blocking 外（settings.rs:34-37）
- 移入 spawn_blocking 闭包内部

【TE-14】settings 命令包装层补测
- save_settings/load_settings 命令包装单测：参数透传、错误映射、浅合并行为（不擦除其他段）

【BE-15】notify_watch 持池锁创建 watcher（notify/mod.rs:233-250）
- FileWatcher::start（含 debouncer.watch 阻塞调用）移到池锁外完成，再短暂持锁插入池
- 保持 pause_all_except 语义不变

改完跑：cargo test --manifest-path src-tauri/Cargo.toml settings -- --test-threads=1 与 cargo test --manifest-path src-tauri/Cargo.toml notify -- --test-threads=1 全绿。
`, { label: 'settings-notify-agent' }),
])

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程，ConPTY 并发 spawn 死锁）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 4 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的全量测试执行结果，测试类断言据此判定（无需重跑）：
---
${testResult ?? '（测试 agent 未返回——测试类断言全部判 not_fixed）'}
---
返回 JSON：{ "allFixed": true/false, "failedItems": ["未通过项ID"], "details": { "<ID>": { "status": "fixed|not_fixed|partial", "evidence": "..." } } }
`, { label: 'verify all items', schema: {
  type: 'object',
  properties: {
    allFixed: { type: 'boolean' },
    failedItems: { type: 'array', items: { type: 'string' } },
    details: { type: 'object' }
  },
  required: ['allFixed', 'failedItems', 'details']
}})

const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
