# Review2 · Stage 03 后端加固（BE-01/02/03/04/06）

> commit: 89988d6（任务简报所给 4a6457f 在全仓不存在——`git cat-file -t 4a6457f` fatal；Stage 03 实际 commit 为 89988d6，message 与 stages.md 约定逐字一致）；核验维度：保真度/实现质量/新引入问题；范围：Stage diff 面 + 触碰文件全量复查。
> 仅登记问题，不分严重度；已核验通过项不列出。
>
> 实跑证据摘要：rg 断言 6 条（`.join()` 仅 thread_join.rs:24；KILL_JOIN_TIMEOUT 零命中 exit 1；跨模块 reader join import 零命中 exit 1；BE-04 守卫 exit 1；git/mod.rs 仅 :55 CP-008 登记行命中（:185 已删）；重采样计数=4≥3）；前端 diff（d1a6838..89988d6，src/ 代码文件）为空；L1 定向全绿：read_dir 23 例 / join 7 例 / git 11 例 / scan_bench 1 例（4.41s，单跑）。

## 问题清单

### R2-BE-01 · test-exemptions.md CP-011 行用例计数失真（「join_with_timeout 4 例」实为 3 例）
- **关联修复项**: BE-01
- **位置**: .claude/test-exemptions.md:14
- **问题**: BE-01 把 reader.rs 1 例 + spawn.rs 3 例去重随迁为 thread_join.rs `join_tests` 3 例后，CP-011 豁免行兜底层级仍写「`join_with_timeout` 4 例」；同文件 :24（TQ-COV-03 行）写「3 例」——BE-01 前 4 例为真值、两行本就互斥，BE-01 后 :14 行过时且两行冲突未收敛。checklist BE-01 的文档同步步骤（第 10/11 条）未指派 test-exemptions.md，同步漏项。
- **证据**: .claude/test-exemptions.md:14「`plan_cleanup_after_join_timeout` 2 例锁死（reader.rs）+ `join_with_timeout` 4 例」；thread_join.rs:35-66 实读仅 3 例（finished/blocked/abandoned）；:24 行写 3 例。
- **建议**: :14 行计数改 3 例，与 :24 行口径对齐。

### R2-BE-02 · notify 测试 shutdown() 换装后丢失 watcher 线程 panic 检测
- **关联修复项**: BE-01
- **位置**: src-tauri/src/notify/mod.rs:972-981（对照 thread_join.rs:24）
- **问题**: 原 `self.handle.join().unwrap()` 在 watcher 线程 panic 时以 Err 返回 → unwrap 失败 → 用例红；换装后 `join_with_timeout` 内部 `let _ = handle.join()` 吞掉 JoinError，panicked 线程 `is_finished()`=true 即返回 true → assert 恒过，panic 检测面丢失。影响有限（event_loop panic 时同用例前置 emit 断言通常先红），但严格度确较改动前下降，属换装引入的测试弱化。
- **证据**: diff 删行 `self.handle.join().unwrap()`；thread_join.rs:23-25 `if handle.is_finished() { let _ = handle.join(); return true; }`——JoinError 被丢弃；对照 settings.rs:528-545 换装（结果经 mpsc 通道回传 + `rx.recv().unwrap().unwrap()`）保留了等价传播，notify 处未采用同等手法。
- **建议**: notify shutdown() 照 settings.rs 手法回传线程 Result，或 join_with_timeout 增加返回 panic 信息的变体供测试用。

### R2-BE-03 · fs 游标解码失败路径（含 BE-02 新增 tag/NUL 校验分支）全量无用例
- **关联修复项**: BE-02（含存量成分）
- **位置**: src-tauri/src/fs/mod.rs:78-92
- **问题**: `decode_page_cursor` 的 Validation 三分支（base64 失败 / UTF-8 失败 / 无 NUL 或 tag 非 D|F）零测试覆盖；tag 校验与 NUL split 是 BE-02 新增逻辑，按硬约束 #11 属「改动的代码」应补测。旧序号游标时代该路径同样无覆盖（存量缺口），本次改写未顺带补上。前端 opaque 契约下解码失败是消费者唯一可观测的错误面，回归无网。
- **证据**: rg `无效目录分页游标|decode_page_cursor` src-tauri/tests/ 零命中；read_dir_tests（fs/mod.rs:786-1201）全部用例只传合法游标或 None。
- **建议**: read_dir_tests 补 1-2 例：非法 base64 游标 / 手工构造 base64("X\0x") 坏 tag 游标 → 断言返回 Validation。

### R2-BE-04 · BE-02 实现与 checklist 写死步骤偏差：601 夹具改 1201（已留痕，测试更强）
- **关联修复项**: BE-02
- **位置**: src-tauri/src/fs/mod.rs:1026-1049
- **问题**: checklist BE-02 步骤 6 写死「601 文件目录拉首页（500）→ …」两步链；实现改 1201 文件三页链。commit body 已留痕理由（601 时第二页即末页、无法验证续页游标链跨页传导），且三页链对「游标前新增不重放 + 游标后新增不漏 + 跨页无重无漏」的锁定确实更强——按保真度维度登记为偏差项，不计缺陷。
- **证据**: checklist.md:295（601 写死）；fs/mod.rs:1028「seed_files(dir.path(), 1201); // 500 + 500 + 201 → 三页，续页游标链完整」；commit 89988d6 body「BE-02：……（1201 文件三页链，601 时第二页即末页无法验续页）」。
- **建议**: 无需修复；如严格对照清单执行，可在 checklist 条目上补一行偏差批注。

### R2-BE-05 · spawn.rs 测试组注释「BE-06」标签与本计划 BE-06 编号撞车
- **关联修复项**: —（存量标签，随本次 diff 改写保留）
- **位置**: src-tauri/src/pty/spawn.rs:2155
- **问题**: 注释「BE-06: join_with_timeout 用例已随迁 crate::thread_join::join_tests（BE-01）」中的 BE-06 指历史修复轮编号（该测试组当年由彼轮 BE-06 引入），与本修复计划 BE-06（scan.rs 基准两轮制）同号不同义；根 CLAUDE.md 短标识符解码规则要求就近定义，pty/CLAUDE.md 无此历史编号解码，读者易误认为基准加固动了 spawn 测试组。
- **证据**: spawn.rs:2155 实读；本计划 checklist BE-06（checklist.md:409）= CP-007 基准加固，与 join 测试无关。
- **建议**: 注释改历史标签为文字描述（如「历史 BE-06 轮」）或一并写清编号所属轮次。

### R2-BE-06 · FileWatcher::drop 复制 stop() 逻辑而非委托（存量不一致，BE-01 双侧同改未收敛）
- **关联修复项**: —（存量问题）
- **位置**: src-tauri/src/notify/mod.rs:166-176 与 :300-313
- **问题**: `HookSignalWatcher::drop` 委托 `self.stop()`（hooks/watcher.rs:146-150），`FileWatcher::drop` 却逐行复制 stop 的信号发送 + join 逻辑。BE-01 换装时两侧各插一遍同款代码 + 同一 warn 文案，已属手工对齐；未来改 stop 语义（如超时值、warn 文案、句柄回收策略）极易漏改 Drop 侧。
- **证据**: notify/mod.rs:167-176（stop）vs :301-312（Drop）——结构、调用、warn 字符串逐字相同；对照 hooks/watcher.rs:148 `fn drop(&mut self) { self.stop(); }`。
- **建议**: FileWatcher::drop 改调 `self.stop()`（stop 已幂等：Option::take 双保险）。
