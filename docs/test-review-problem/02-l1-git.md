# 02 L1 git 测试 Review

## 元信息

- **领域**: Rust 后端 git 模块（`src-tauri/src/git/mod.rs`）
- **测试位置**: `src-tauri/src/git/mod.rs` 内 `#[cfg(test)] mod tests`
- **用例数**: 88 条（`grep '#\[test\]'` 口径）
- **源码规模**: 2718 行（生产代码 1–582 行，测试代码 584–2718 行）
- **覆盖率概况**: 行覆盖 1550/1813 = **85.5%**；未覆盖 263 行，其中生产代码未覆盖约 120 行
- **审查日期**: 2026-08-04
- **审查人**: Claude Code（静态审查，未运行测试）

## 覆盖率缺口

按业务风险分级。生产代码行号范围 1–582；测试辅助/断言未命中行号位于 `#[cfg(test)]` 内，属“既定豁免”，不列入真缺口。

### 🔴 核心逻辑零覆盖

| 位置 | 说明 | 风险 |
|------|------|------|
| `src-tauri/src/git/mod.rs:42–43` | `status_to_str` 的 `status.is_conflicted() → "conflict"` 分支 | Commit 视图对冲突文件需着色为 conflict；该分支未测 |
| `src-tauri/src/git/mod.rs:127–209` | `git_status` Tauri 命令包装函数（含路径沙箱、RwLock、spawn_blocking、old_path 拼接） | 前端实际调用的命令；全部未覆盖 |
| `src-tauri/src/git/mod.rs:215–246` | `git_diff` Tauri 命令包装函数 | diff 面板核心入口；未覆盖 |
| `src-tauri/src/git/mod.rs:395–454` | `git_file_at_head` Tauri 命令包装函数 | gitshow/diff 左栏核心入口；未覆盖 |
| `src-tauri/src/git/mod.rs:461–533` | `git_rollback` Tauri 命令包装函数 | Commit 视图右键“回滚”核心入口；未覆盖 |
| `src-tauri/src/git/mod.rs:540–582` | `git_unstage` Tauri 命令包装函数 | Commit 视图右键“删除 added 文件”核心入口；未覆盖 |
| `src-tauri/src/git/mod.rs:315–320` | `compute_diff_hunks` 中“修改后多余新增行”分支（`ac > shared` 推 added hunk） | diff  gutter 对“替换+插入”场景会漏标绿色新增 |
| `src-tauri/src/git/mod.rs:361–363` | `compute_diff_hunks` 中 `prev_was_del` 触发 flush 的分支 | 连续删除组后接新增组的 hunk 拆分可能异常 |

### 🟡 边界分支未覆盖

| 位置 | 说明 | 风险 |
|------|------|------|
| `src-tauri/src/git/mod.rs:264–265` | `compute_diff_hunks` 中 `repo.head()` 返回非 `UnbornBranch` 的其他 Err | 罕见但会 panic/误报；无回归用例 |
| `src-tauri/src/git/mod.rs:83–98` | `get_or_open_repo` 缓存命中后 `validate_path_within_root(&root, &wd)?` 失败分支 | project_root 切换后旧缓存项理论上应被拒 |
| `src-tauri/src/git/mod.rs:101–102,119–121` | `get_or_open_repo` discover 失败 / 重新打开失败分支 | 非 git 目录、bare repo 仅部分覆盖 |
| `src-tauri/src/git/mod.rs:84–86,112–114` | `Mutex` 锁被 poison 时的错误映射分支 | 低概率，但 panic 恢复路径未验证 |
| `src-tauri/src/git/mod.rs:485–491,497–499,503–508,511–512,516–524` | `git_rollback` 中 HEAD 不存在 / tree 条目失败 / 写文件失败 / index 操作失败等 Err 分支 | 异常路径无回归 |
| `src-tauri/src/git/mod.rs:562–564,567–569,571–573,575–577,579–580` | `git_unstage` 中 index 获取/移除/写入失败分支 | 异常路径无回归 |
| `src-tauri/src/git/mod.rs:447` | `String::from_utf8_lossy` 对非 UTF-8 blob 的转换结果未断言 | 二进制文件当作文本返回乱码，但无测试 |

### 🟢 低风险未覆盖

- `git_status` 中对 `entry.path()` 为空串的降级路径（161–165 行 `unwrap_or("")`）
- `git_status` 中 `.gitignore` 自身作为 untracked 的精确路径断言（实际测试已覆盖其行为）
- `git_diff` 中 `repo_path.is_empty()` 走 `file_path` 分支（通过子目录测试间接覆盖，但未显式断言）

### 既定豁免

- 所有位于 `#[cfg(test)]` 区域（行号 ≥ 584）的未覆盖辅助闭包、失败消息格式化字符串、panic 路径——属于测试代码自身的分支，不计入生产覆盖缺口。

## 问题列表

### P-1 [🔴] [断言有效性] 大量 Tauri 命令测试未调用被测函数，而是 inline 重写底层 git2 逻辑

- **位置**:
  - `git_rollback_restores_modified` 2227–2265
  - `git_rollback_restores_deleted` 2269–2295
  - `git_rollback_autocrlf_clean_status` 2299–2329
  - `git_rollback_paths_isolation` 2333–2358
  - `git_rollback_cross_instance_clean` 2385–2416
  - `git_rollback_two_step_restores_modified` 2421–2460
  - `git_rollback_two_step_restores_deleted` 2464–2492
  - `git_rollback_two_step_autocrlf_clean` 2496–2528
  - `git_rollback_two_step_paths_isolation` 2532–2559
  - `git_rollback_cross_instance_status_clean` 2563–2597
  - `git_unstage_index_new_file` 2627–2665
  - `git_unstage_remove_path_on_index_modified_removes_entry` 2670–2703
  - `git_file_at_head_reads_content` 2136–2143
  - `git_file_at_head_subdirectory_file` 2176–2193
  - `git_file_at_head_deleted_file_roundtrip` 2197–2220
- **代码片段**:
  ```rust
  // git_rollback_restores_modified 中
  let tree = repo.head().unwrap().peel_to_tree().unwrap();
  let entry = tree.get_path(Path::new("a.txt")).unwrap();
  let blob = entry.to_object(&repo).unwrap().peel_to_blob().unwrap();
  std::fs::write(&file_path, blob.content()).unwrap();
  let mut index = repo.index().unwrap();
  index.add_path(Path::new("a.txt")).unwrap();
  index.write().unwrap();
  ```
- **问题**: 这些测试复制了 `git_rollback` / `git_unstage` / `git_file_at_head` 命令函数的内部 git2 调用序列，但**没有调用被测的 Tauri 命令函数**。它们测的是“git2 API 能按预期工作”，而不是“我们的命令函数能正确协调路径沙箱、缓存、错误映射和 spawn_blocking”。
- **改法**:
  1. 新增对命令函数本身的集成测试：构造 `AppState`（可用 `Default::default()` 或测试专用构造器），调用 `git_rollback(repo_path, file_path, state).await` 等，再断言结果/文件状态。
  2. 若 Tauri `State` 在单元测试中难以构造，至少把命令函数体内的核心逻辑拆为可独立测试的纯函数/同步函数（如 `rollback_in_spawn_blocking(repo, workdir, file_path)`），并在测试中直接调用该函数。
  3. 保留现有 git2 行为测试作为“底层原语”测试，但明确标注其不覆盖命令层。
- **变异推演**: 若把 `git_rollback` 中的 `std::fs::write(&file_path, blob.content())` 误写成 `std::fs::write(&file_path, "")`，上述 2227/2269 等测试不会变红（它们自己写 blob），因此这些测试**无法守护生产命令函数**。

### P-2 [🔴] [断言有效性] `git_rollback_two_step_*` 系列测试与当前生产实现脱节

- **位置**:
  - `git_rollback_two_step_restores_modified` 2421–2460
  - `git_rollback_two_step_restores_deleted` 2464–2492
  - `git_rollback_two_step_autocrlf_clean` 2496–2528
  - `git_rollback_two_step_paths_isolation` 2532–2559
  - `git_rollback_cross_instance_status_clean` 2563–2597
  - `git_rollback_two_step_nonexistent_in_tree_no_panic` 2601–2609
  - `git_rollback_two_step_unborn_branch_errors` 2613–2621
- **代码片段**:
  ```rust
  let head = repo.head().unwrap().peel_to_commit().unwrap();
  repo.reset_default(Some(head.as_object()), &["a.txt".to_string()]).unwrap();
  let mut checkout = git2::build::CheckoutBuilder::new();
  checkout.force();
  checkout.path("a.txt");
  repo.checkout_index(None, Some(&mut checkout)).unwrap();
  ```
- **问题**: 生产代码 `git_rollback` 已改为 `std::fs::write(blob) + index.add_path` 方案（见 CLAUDE.md 与源码 510–524 行）。这些测试仍验证已废弃的“reset_default + checkout_index”两步法，属于**过时测试**。它们不仅不覆盖当前命令，还会误导维护者认为旧路径仍在使用。
- **改法**: 删除或重写为直接调用当前 `git_rollback` 命令的测试；保留 1–2 条作为历史对照时需显式注释说明“已废弃实现，仅文档目的”。
- **变异推演**: 若把生产 `git_rollback` 的 `index.add_path` 删除，使 status 仍 dirty，这些 two_step 测试仍绿——因为它们验证的是另一条路径。

### P-3 [🔴] [断言有效性] `git_status_non_renamed_old_path_is_none` 是假测试

- **位置**: `src-tauri/src/git/mod.rs:1700–1739`
- **代码片段**:
  ```rust
  let status_flag = entry.status();
  if status_flag.contains(git2::Status::INDEX_RENAMED)
      || status_flag.contains(git2::Status::WT_RENAMED)
  {
      continue; // 跳过 renamed 条目（如预期不应存在）
  }
  // 非 renamed 条目 oldPath 应为 None
  let old_path = if status_flag.contains(git2::Status::INDEX_RENAMED) {
      ...
  } else if status_flag.contains(git2::Status::WT_RENAMED) {
      ...
  } else {
      None
  };
  assert!(old_path.is_none(), "非 renamed 条目 oldPath 应为 None");
  ```
- **问题**: 测试先 `continue` 掉所有 renamed 条目，然后对非 renamed 条目计算 old_path 时，条件分支又要求 status 含 `INDEX_RENAMED`/`WT_RENAMED`，这永远为 false，因此 `old_path` 恒为 `None`。**无论生产代码如何处理非 renamed 条目的 old_path，断言都通过**。
- **改法**: 直接断言 `entry.head_to_index()` / `entry.index_to_workdir()` 返回的 delta 在 non-renamed 时 `old_file().path()` 为 None；或调用生产 `git_status` 命令并断言返回条目的 `old_path` 字段为 None。
- **变异推演**: 若把生产 `git_status` 中所有非 renamed 条目的 `old_path` 强制设为 `Some("bug")`，本测试仍绿。

### P-4 [🔴] [测试覆盖度] 五个 Tauri 命令函数全部未被命令层测试覆盖

- **位置**:
  - `git_status` 127–209
  - `git_diff` 215–246
  - `git_file_at_head` 395–454
  - `git_rollback` 461–533
  - `git_unstage` 540–582
- **问题**: 上述命令是前端 IPC 的唯一入口。现有 88 条测试中，没有任何一条直接 await 调用这些 async 命令函数。覆盖缺口包括：
  - `State<AppState>` 的 project_root 读写锁路径
  - `validate_path_within_root` 在命令层的拒绝行为
  - `tokio::task::spawn_blocking` 的 join 错误映射
  - 命令特定的错误消息契约（如“HEAD 中不存在”）
- **改法**: 参考 `src-tauri/src/hooks/usage.rs` 的 `hooks_context_usage` 端到端测试模式，构造最小 `AppState` 并在测试中 await 调用命令函数。新增：
  - `git_status_command_returns_entries`
  - `git_diff_command_returns_hunks`
  - `git_file_at_head_command_reads_content`
  - `git_rollback_command_restores_file`
  - `git_unstage_command_removes_index_entry`
  - 路径沙箱拒绝用例（如传入 `file_path` 在 project_root 外）
- **变异推演**: 若把 `git_status` 中的 `old_path` 拼接逻辑改为使用错误 workdir（如用 `repo_path` 而非 `workdir`），现有测试不会变红，因为没有任何命令层测试断言返回的 JSON。

### P-5 [🟡] [测试覆盖度] `status_to_str` 的 conflict 分支未覆盖

- **位置**: `src-tauri/src/git/mod.rs:42–43`
- **代码片段**:
  ```rust
  if status.is_conflicted() {
      Some("conflict")
  ```
- **问题**: `test_status_to_str_all_flags` 覆盖了 WT_NEW/INDEX_NEW/modified/deleted/renamed/ignored/CURRENT，但**没有 conflict**。Commit 视图对 conflict 文件需要正确着色，缺失该分支测试。
- **改法**: 在 `test_status_to_str_all_flags` 的 cases 中加入 `(git2::Status::CONFLICTED, Some("conflict"))`；并补一个真实构造合并冲突的集成测试。
- **变异推演**: 若把 `status.is_conflicted()` 判断删除或改为返回 `"modified"`，当前所有测试仍绿。

### P-6 [🟡] [测试覆盖度] `compute_diff_hunks` 关键边界分支未覆盖

- **位置**:
  - `src-tauri/src/git/mod.rs:315–320`（多余新增行）
  - `src-tauri/src/git/mod.rs:361–363`（prev_was_del flush）
  - `src-tauri/src/git/mod.rs:264–265`（非 UnbornBranch 的 HEAD 错误）
- **问题**:
  - `line_callback_modified_plus_extra_additions` 1807 仅验证“存在 added hunk”，未精确断言其 `old_start=0 / old_lines=0 / new_start / new_lines`，因此 315–320 行未真正执行到。
  - 361–363 行处理“连续 '-' 组后接新的 '-' 且前面有未配对 '+'”的 flush，无对应测试。
  - 264–265 行处理 `repo.head()` 除 UnbornBranch 外的其他错误，无测试。
- **改法**:
  - 新增 `git_diff_precise_replace_and_insert`：将 1 行替换为 1 行并在其后插入 2 行，断言返回 1 个 modified hunk + 1 个 added hunk，且 added hunk 的 `old_start=0`。
  - 新增 `git_diff_precise_delete_groups_then_add`：构造 `-a -b +c` 类 diff，触发 361–363 flush。
  - 新增 `git_diff_head_error_not_unborn`：通过 corrupt refs 或锁文件模拟 HEAD 读取失败，断言返回 Err。
- **变异推演**: 删除 315–320 行代码后，`line_callback_modified_plus_extra_additions` 仍通过（它只检查存在 added hunk，不检查具体字段）。

### P-7 [🟡] [用例设计质量] 环境隔离不完整：依赖系统 git 全局配置

- **位置**:
  - `init_temp_repo` 594–620
  - `commit_file` 623–636
  - 所有使用 `Command::new("git")` 的测试
- **问题**: 测试使用系统 git CLI 创建 commit。虽然部分 rollback 测试显式设置 `core.autocrlf`，但：
  - `init_temp_repo` 未设置 `core.autocrlf`、`core.safecrlf`、`init.defaultBranch` 等全局配置隔离
  - 用户本地若 `core.autocrlf=input` 或 `init.defaultBranch=main` 之外的分支策略，可能导致测试不稳定
  - `commit_file` 写入 `\n` 结尾内容，在 `core.autocrlf=true` 全局环境下，git commit 可能把 blob 存为 CRLF，与后续断言冲突
- **改法**: 在 `init_temp_repo` 中统一设置局部 git config：
  ```rust
  Command::new("git").args(["config", "core.autocrlf", "false"]).current_dir(&path).output().unwrap();
  Command::new("git").args(["config", "core.safecrlf", "false"]).current_dir(&path).output().unwrap();
  Command::new("git").args(["config", "init.defaultBranch", "main"]).current_dir(&path).output().unwrap();
  ```
  需要 autocrlf=true 场景的测试再单独覆盖。
- **变异推演**: 不适用（稳定性问题）。

### P-8 [🟡] [用例设计质量] `git_status` 状态测试使用弱断言 `any(...)`，未精确验证条目

- **位置**:
  - `git_status_modified_file` 691–707
  - `git_status_untracked_file` 710–723
  - `git_status_added_file` 726–740
  - `git_status_deleted_file` 743–759
  - `git_status_includes_modified_tracked` 820–837
- **问题**: 这些测试仅验证“statuses 中存在某个 flag 的条目”，未断言：
  - 条目路径正确
  - 状态字符串与 `status_to_str` 映射一致
  - 条目数量符合预期
- **改法**: 将断言改为对 `statuses.iter()` 收集后的 Vec 做精确检查，例如：
  ```rust
  let entries: Vec<_> = statuses.iter().map(...).collect();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].path, "test.txt");
  assert!(entries[0].status.contains(WT_MODIFIED));
  ```
  同时补一条端到端断言：调用生产 `git_status` 命令返回的 JSON 中 `status="modified"`。
- **变异推演**: 若把 `status_to_str` 中 WT_MODIFIED 映射改为 `"added"`，`git_status_modified_file` 仍可能通过（只要 INDEX_NEW 等其它条目也在集合中）。

### P-9 [🟡] [用例设计质量/冗余] 测试名称与测试内容不一致，造成维护困惑

- **位置**:
  - `git_diff_returns_hunks` 1109–1145：实际只验证 hunk 数量 >0 且 old_lines/new_lines ≥1，未验证 hunk 内容
  - `git_diff_new_file_no_head` 1148–1161：名为 diff 测试，实际只验证 `repo.head()` 返回 UnbornBranch
  - `git_diff_added_lines_hunk` 1194–1227：只验证 `new_lines > old_lines`，未验证 added hunk 结构
  - `git_diff_deleted_lines_hunk` 1230–1262：只验证 `old_lines > new_lines`，未验证 deleted hunk 结构
- **问题**: 这些属于早期 B2/P0 阶段的“git2 行为摸底”测试，命名暗示精确 diff hunk 验证，实际只做了存在性/弱不等式断言。与后续 `git_diff_precise_*` 系列重复或模糊。
- **改法**: 合并或重命名：
  - 将 `git_diff_returns_hunks` 改名为 `git_diff_hunk_count_positive`
  - 将 `git_diff_new_file_no_head` 改名为 `repo_head_unborn_branch`
  - 或删除这些弱断言测试，用 `git_diff_precise_*` 系列替代
- **变异推演**: 不适用（命名/冗余问题）。

### P-10 [🟡] [稳定性风险] `git_status_excludes_ignored_files` 等测试依赖 `.gitignore` 生效时序

- **位置**:
  - `git_status_excludes_ignored_files` 771–797
  - `git_status_includes_untracked_not_ignored` 800–817
  - `git_status_includes_modified_tracked` 820–837
  - `git_status_tracked_then_ignored_still_shows_status` 840–862
- **问题**: 测试写入 `.gitignore` 后立即调用 `repo.statuses()`。git2 的 `.gitignore` 解析依赖目录扫描缓存，在慢 IO 或高并发场景下，新写入的 `.gitignore` 可能未被立即识别（虽然实践中概率低）。
- **改法**: 在写入 `.gitignore` 后显式触发 git2 的 ignore 缓存刷新，或至少通过 `repo.statuses(None)` 调用一次预热后再断言。更稳健的做法是用 `git2::Repository::add_ignore_rule` 在内存中设置规则，避免磁盘时序。
- **变异推演**: 不适用（稳定性问题）。

### P-11 [🟢] [结构与可维护性] `ci_l1_uses_single_test_thread` 不属于 git 领域测试

- **位置**: `src-tauri/src/git/mod.rs:2000–2011`
- **代码片段**:
  ```rust
  fn ci_l1_uses_single_test_thread() {
      let ci = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../.github/workflows/ci.yml")).unwrap();
      assert!(ci.contains("--test-threads=1"), ...);
  }
  ```
- **问题**: 该测试验证 CI 配置字符串，与 git 业务无关；放在 git/mod.rs 中造成领域污染。它同时引入对 `.github/workflows/ci.yml` 文件路径的硬编码依赖。
- **改法**: 迁移到独立的 `src-tauri/tests/ci_config_tests.rs` 或项目级测试目录，并在 test-inventory 中单独登记。
- **变异推演**: 不适用。

### P-12 [🟢] [结构与可维护性] 单文件 88 条测试，setup 工厂与测试逻辑混合

- **位置**: `src-tauri/src/git/mod.rs:584–2718`
- **问题**:
  - `init_temp_repo`/`commit_file`/`git_add` 与测试用例混在一起
  - 没有按命令拆分测试模块（如 `mod status_tests; mod diff_tests; mod rollback_tests;`）
  - `read_file_at_head` 辅助函数（2107–2133）inline 复制了生产代码
- **改法**: 按命令拆分为独立测试文件（`tests/git_status_tests.rs`、`tests/git_diff_tests.rs` 等），或使用 `mod tests { mod status_tests { ... } }` 嵌套；将 `init_temp_repo` 等工厂提取到 `src-tauri/src/test_utils/git_repo.rs` 供多个模块复用。
- **变异推演**: 不适用。

### P-13 [🟡] [Mock 使用合理性] 使用系统 git CLI 提交但未校验 git 版本

- **位置**: 所有调用 `Command::new("git")` 的测试
- **问题**: 测试依赖系统 `git` 可执行文件。不同 git 版本在以下行为上可能有差异：
  - `git mv` 的 rename 检测阈值
  - `git status` 默认是否启用 rename 检测
  - `core.autocrlf` 默认行为
- **改法**: 在 CI 文档中声明最低 git 版本（如 ≥2.30）；或在 `init_temp_repo` 中显式关闭所有可能干扰的全局配置，确保测试自包含。
- **变异推演**: 不适用。

### P-14 [🟡] [断言有效性] `git_file_at_head_unborn_branch_err` 未调用被测函数

- **位置**: `src-tauri/src/git/mod.rs:2145–2157`
- **代码片段**:
  ```rust
  let result = repo.head();
  match result {
      Err(e) if e.code() == git2::ErrorCode::UnbornBranch => { ... }
      _ => panic!("空仓库应返回 UnbornBranch"),
  }
  ```
- **问题**: 该测试只验证 `git2::Repository::head()` 在空仓库返回 UnbornBranch，没有调用 `git_file_at_head` 命令，也没有验证命令返回的错误消息含“HEAD 中不存在”。
- **改法**: 改为调用生产 `git_file_at_head` 命令并断言 `Err` 消息包含“HEAD 中不存在”。
- **变异推演**: 若把 `git_file_at_head` 的 UnbornBranch 错误消息改成“unknown error”，本测试仍绿。

### P-15 [🟡] [测试覆盖度] 路径沙箱失败分支未覆盖

- **位置**:
  - `git_status` 139 行 `validate_path_within_root(&root, Path::new(&repo_path))?`
  - `git_diff` 229–231 行 `validate_path_within_root`
  - `git_file_at_head` 407 行 `validate_path_within_root(&root, Path::new(&file_path))?`
  - `git_rollback` 472 行 `validate_path_within_root(&root, Path::new(&file_path))?`
  - `git_unstage` 550 行 `validate_path_within_root(&root, Path::new(&file_path))?`
- **问题**: 所有命令在传入 `project_root` 外的路径时应返回 `PathNotAllowed`。现有测试要么不调用命令函数，要么只传合法路径，未覆盖拒绝分支。
- **改法**: 为每个命令新增路径沙箱拒绝用例。例如构造 `AppState { project_root: Some("/tmp/project".into()), ... }`，然后传入 `/etc/passwd` 作为 `file_path`，断言返回 `AppError::PathNotAllowed`。
- **变异推演**: 若把 `git_status` 中的 `validate_path_within_root` 调用删除，当前所有测试仍绿。

## 已做变异推演的用例清单

| 用例 | 推演的变异 | 是否会变红 | 原因 |
|------|-----------|-----------|------|
| `test_status_to_str_all_flags` | 删除 conflict 分支 / 把 WT_MODIFIED 映射改为 "added" | 改 WT_MODIFIED 会红；删 conflict 不会红 | conflict 分支本身未覆盖，但其它映射被表驱动覆盖 |
| `git_status_modified_file` 等 B2 用例 | 把 `status_to_str` 中 WT_MODIFIED 改为 "added" | 大概率仍绿 | 只断言存在 WT_MODIFIED flag，不断言 status 字符串 |
| `git_status_non_renamed_old_path_is_none` | 非 renamed 条目 old_path 强制返回 Some("bug") | 仍绿 | 计算 old_path 的条件分支永远不会命中 |
| `git_diff_precise_single_line_modification` | `compute_diff_hunks` 中 shared 计算 `>` 改为 `>=` | 可能仍绿 | 单行修改时 `dc=ac=1`，`>=` 结果相同 |
| `git_diff_precise_consecutive_additions_merged` | 删除“纯新增 hunk”分支（332–337） | 变红 | 断言 `old_lines=0` 会失败 |
| `line_callback_modified_plus_extra_additions` | 删除 315–320 多余新增行分支 | 仍绿 | 只断言存在 added hunk，不断言其 `old_start=0` |
| `get_or_open_repo_cache_no_false_hit_for_subrepo` | 把 cache hit 条件 `search.starts_with(workdir)` 反向 | 变红 | 父目录访问会命中子仓库缓存，workdir 断言失败 |
| `get_or_open_repo_bare_repo_returns_err` | 删除 bare repo 的 workdir None 检查 | 变红 | 断言 is_err 会失败 |
| `git_rollback_restores_modified` | 生产 `git_rollback` 写空内容而非 blob | 仍绿 | 测试自己写 blob，不调用命令函数 |
| `git_rollback_two_step_restores_modified` | 生产 `git_rollback` 删除 index.add_path | 仍绿 | 测试使用已废弃的两步法 |
| `git_file_at_head_reads_content` | 生产 `git_file_at_head` 返回 `""` | 仍绿 | 测试使用辅助函数 read_file_at_head |
| `git_unstage_index_new_file` | 生产 `git_unstage` 不调用 index.write | 仍绿 | 测试直接调用 git2 index API |
| `git_file_at_head_unborn_branch_err` | 生产 `git_file_at_head` 错误消息改变 | 仍绿 | 只验证 git2 行为，未验证命令错误消息 |
| 路径沙箱相关命令用例 | 删除命令中的 `validate_path_within_root` | 仍绿 | 没有命令层沙箱拒绝测试 |

## 小结

- **🔴 4 条**、**🟡 9 条**、**🟢 2 条**，共 **15 条问题**。
- 最核心的问题是：**五个 Tauri 命令函数（git_status/git_diff/git_file_at_head/git_rollback/git_unstage）几乎没有命令层测试**，现有测试大量 inline 重写底层 git2 逻辑或测试已废弃实现。这导致路径沙箱、错误映射、spawn_blocking、缓存命中等命令层行为处于无回归守卫状态。
- 其次，`status_to_str` 的 conflict 分支、`compute_diff_hunks` 的“多余新增行”和“prev_was_del flush”分支存在真缺口。
- 建议优先级：先补命令层集成测试 → 修正假测试 P-3 → 删除/重写过时 two_step 测试 → 补 conflict 与 diff 边界分支。
