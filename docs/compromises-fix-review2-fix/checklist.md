# compromises-fix-review2 修复清单（30 项）

> 输入：`docs/compromises-fix-review2/` 二轮 review 30 条问题（7 份分文件 + summary.md）。
> 用户裁决：30 条全修；历史执行档修订一律「落地复核注记追加，不改写原文」；review-04 R2-FE-03 修代码加超时兜底；review-05 R2-FE-04 改比对实现（抽样指纹）；review-05 R2-BE-01（skill 收尾约定）纳入。
> 组织方式：不用 P0-P4 优先级——执行序由 Stage 依赖顺序表达（见 stages.md）；编号 = 模块前缀 + 序号，来源留痕列指向 review 文件原编号（review 编号为文件内独立序列、跨文件撞号，引用须带文件名消歧）。

## 事实核验留痕（计划期实读修正 review 报告漂移）

- review-02 R2-SEC-03 称断言在「verify/stage-02.md:13 + checklist.md:160」两处——实读 verify/stage-02.md 全文无该断言（:13 为 SEC-03 syncWarned 断言），仅 checklist.md:161 一处。
- review-04 R2-FE-01 实读为 use-file-tree.test.ts:1037-1040（waitFor + toHaveBeenCalled + lastPayload）。
- knip.json:14 另有前轮 `docs/compromises-fix/**` ignore（不在本轮范围，不动）；:15 才是本轮收窄对象。
- 全部修复点现状代码均计划期实读（行号 + 代码形态），未凭 review 转述。

## 编号总表

| 新 ID | 来源 | 问题 | Stage |
|---|---|---|---|
| TE-01 | review-01 R2-TE-01 | ts7 日志标签漏 HTTP | 01 |
| TE-02 | review-01 R2-TE-02 | vitest exclude 死配置 | 01 |
| TE-03 | review-06 R2-TE-01 | WARN 跨 chunk 漏计 | 01 |
| TE-04 | review-06 R2-TE-02 | fallback exit 未 ?? 1 | 01 |
| FE-06 | review-04 R2-FE-06 | knip ignore 过宽 | 01 |
| SEC-01 | review-02 R2-SEC-01 | CSP 测试缺 script/style 断言 | 02 |
| SEC-04 | review-02 R2-SEC-04 | 「下轮轮询自愈」注释失实 | 02 |
| BE-02 | review-03 R2-BE-02 | shutdown 丢 panic 检测 | 03 |
| BE-06 | review-03 R2-BE-06 | Drop 复制非委托 | 03 |
| BE-03 | review-03 R2-BE-03 | 游标解码三分支零用例 | 03 |
| BE-05 | review-03 R2-BE-05 | spawn.rs BE-06 撞号 | 03 |
| BE-01 | review-03 R2-BE-01 | exemptions 计数 4→3 | 03 |
| FE-02 | review-04 R2-FE-02 | 无快照路径双提交 | 04 |
| FE-01 | review-04 R2-FE-01 | 「恰好一次」未锁 | 04 |
| FE-03 | review-04 R2-FE-03 | 抑制态无超时兜底 | 04 |
| FE-04 | review-04 R2-FE-04 | 注释漏第 4 解除点 | 04 |
| FE-07 | review-05 R2-FE-01 | inflight 旧代际被消费 | 05 |
| FE-09 | review-05 R2-FE-03 | 首挂 stat 基线竞态 | 05 |
| FE-10 | review-05 R2-FE-04 | 同 size/mtime 假阴性→指纹 | 05 |
| FE-08 | review-05 R2-FE-02 | 重载无大小防线 | 05 |
| SEC-02 | review-02 R2-SEC-02 | L4 双 spec 未完整执行注记 | 06 |
| SEC-03 | review-02 R2-SEC-03 | catch 零命中断言字面不成立 | 06 |
| BE-04 | review-03 R2-BE-04 | 601 夹具偏差批注 | 06 |
| FE-05 | review-04 R2-FE-05 | stages/verify 路径错误 | 06 |
| DOC-03 | review-07 R2-DOC-03 | 进度表计数矛盾 | 06 |
| DOC-04 | review-07 R2-DOC-04 | .temp/node22 断言恒假 | 06 |
| DOC-01 | review-07 R2-DOC-01 | 「无 CSP」漏改 5 处 | 07 |
| DOC-02 | review-07 R2-DOC-02 | adr/exemptions 无注记 | 07 |
| DOC-05 | review-05 R2-BE-01 | skill 收尾约定补登记纪律 | 07 |
| TE-05 | review-06 R2-TE-03 | workspace 文档签名过期 | 07 |

---

## TE-01（来源：review-01 R2-TE-01）ts7-trigger 日志标签覆盖非 2xx

1. **位置**：`scripts/check-ts7-trigger.mjs:57`
2. **现状**：`console.error(`[ts7-trigger] 查询失败(网络/解析):${e.message}`);`——catch 承接 getJson 三类失败：网络 error 事件 / 非 2xx `reject(new Error(`HTTP ${status}`))`（:33-36）/ JSON.parse 失败；标签漏「HTTP」类。
3. **修复步骤**：:57 标签改 `[ts7-trigger] 查询失败(网络/HTTP/解析):${e.message}`。
4. **测试同步**：无（CI 触发器脚本，无单测锚点；豁免现状不动）。
5. **文档同步**：无。
6. **验证**：`rg "网络/HTTP/解析" scripts/check-ts7-trigger.mjs` 1 命中；`node --check scripts/check-ts7-trigger.mjs` exit 0。

## TE-02（来源：review-01 R2-TE-02）vitest exclude 死配置删除

1. **位置**：`vitest.config.ts:7`
2. **现状**：`exclude: ['node_modules', '.temp', 'e2e-tests']`——include 已收敛 `src/__tests__/**/*.test.{ts,tsx}`，exclude 整段死配置；且自定义 exclude 整体替换 vitest 默认值（`**/node_modules/**` 等），仓根一级 `'node_modules'` 防护弱于默认。
3. **修复步骤**：删除 :7 exclude 行（恢复 vitest 默认 exclude 防护；`.temp`/`e2e-tests` 本不在 include glob 内）。
4. **测试同步**：`npm test` 全量绿且用例计数 = 3261（review2 基线）不缩水即为回归证据。
5. **文档同步**：无。
6. **验证**：`rg "exclude" vitest.config.ts` 零命中；npm test 绿（199 文件/3261 例）。

## TE-03（来源：review-06 R2-TE-01）WARN 计数跨 chunk 缓冲 + UTF-8 截断修复

1. **位置**：`e2e-tests/run-wdio.cjs:36-46`
2. **现状**：

```js
let coreInvokeWarnCount = 0;
function wireWarnCounting(src, dst) {
  src.on('data', (chunk) => {
    const s = chunk.toString();
    coreInvokeWarnCount += (s.match(/core\.invoke not available after 5s/g) ?? []).length;
    dst.write(s);
  });
}
```

chunk 独立匹配——WARN 串跨 chunk 边界时两半均不命中漏计；`chunk.toString()` 切断多字节 UTF-8 序列产生替换字符乱码。该函数被 stdout/stderr 调用两次（:314-315、:351-352），残串状态必须每流独立（闭包内，禁模块级共享）。

3. **修复步骤**：:36-46 整段替换为：

```js
// TE-09: tauri-service "core.invoke not available after 5s" WARN 计数（性能回归感知
// 可观测化——focus 命令面扩大 = 计数显著超基线）。stdio 改 pipe 转发以计数；
// FORCE_COLOR=1 保子进程日志着色（pipe 后非 TTY 失色）
const { StringDecoder } = require('string_decoder');
const WARN_PATTERN = 'core.invoke not available after 5s';
let coreInvokeWarnCount = 0;
function wireWarnCounting(src, dst) {
  const decoder = new StringDecoder('utf8'); // 多字节 UTF-8 跨 chunk 截断防乱码
  let tail = ''; // 上 chunk 尾部残串（模式长度-1 字符）——WARN 跨 chunk 断行拼接计数；每流独立
  src.on('data', (chunk) => {
    const s = decoder.write(chunk);
    coreInvokeWarnCount += ((tail + s).match(/core\.invoke not available after 5s/g) ?? []).length;
    tail = (tail + s).slice(-(WARN_PATTERN.length - 1));
    dst.write(s);
  });
  src.on('end', () => {
    const rest = decoder.end();
    if (rest) dst.write(rest);
  });
}
```

4. **测试同步**：run-wdio.cjs 启动器分支已登记豁免（test-exemptions.md「run-wdio.cjs 启动器分支」行，兜底 = L4 全量 e2e）；不新增单测锚点。
5. **文档同步**：无（e2e-tests/CLAUDE.md TE-09 节语义不变）。
6. **验证**：`rg "StringDecoder" e2e-tests/run-wdio.cjs` 命中；`node --check e2e-tests/run-wdio.cjs` exit 0；语义式：Read 确认 tail 拼接逻辑且残串在函数闭包内（每流独立，非模块级共享）。

## TE-04（来源：review-06 R2-TE-02）fallback 退出码对齐

1. **位置**：`e2e-tests/run-wdio.cjs:355`
2. **现状**：fallback close 回调 `process.exit(code);`——信号杀死时 `code === null`，`process.exit(null)` 实测退化为 exit 0（review-06 实跑实证），wdio 被杀被掩为成功；runWdio 通道 :318 已是 `process.exit(code ?? 1)`。
3. **修复步骤**：:355 改 `process.exit(code ?? 1);`。
4. **测试同步**：无（同 TE-03 豁免行）。
5. **文档同步**：无。
6. **验证**：`rg "process.exit\(code \?\? 1\)" e2e-tests/run-wdio.cjs` 2 命中（:318/:355 区）；`rg "process.exit\(code\)" e2e-tests/run-wdio.cjs` 零命中。

## FE-06（来源：review-04 R2-FE-06）knip ignore 收窄 + 本任务前瞻登记

1. **位置**：`knip.json:15`
2. **现状**：`"docs/compromises-fix-review-fix/**"` 整目录 ignore（压 8 项 workflows/*.js unused 红）——此后该目录任何新增脚本不再产生 knip 信号；:14 另有前轮 `docs/compromises-fix/**`（不在本轮范围，不动）。
3. **修复步骤**：:15 收窄为 `"docs/compromises-fix-review-fix/workflows/*.js"`；同时追加 `"docs/compromises-fix-review2-fix/workflows/*.js"`（本任务产物前瞻登记——workflow 脚本落盘即入 git，knip --production 会扫到）。
4. **测试同步**：无。
5. **文档同步**：无。
6. **验证**：`rg "compromises-fix-review-fix/\*\*" knip.json` 零命中；`npx knip --production` exit 0（本任务 workflows 落盘后复跑仍 exit 0）。

## SEC-01（来源：review-02 R2-SEC-01）CSP 测试补 script/style 断言

1. **位置**：`src-tauri/src/preview.rs:604-611`（`host_page_carries_domain_csp`）
2. **现状**：四断言锁 `Content-Security-Policy` / `default-src 'none'` / `img-src data:` / `font-src data:`——HOST_PAGE CSP 实际含五指令（`default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:`），script/style 两指令无断言锁，回归无网。
3. **修复步骤**：用例内追加两行：

```rust
assert!(HOST_PAGE.contains("script-src 'unsafe-inline'"));
assert!(HOST_PAGE.contains("style-src 'unsafe-inline'"));
```

4. **测试同步**：即该用例自身；L1 定向 `cargo test --test lib_tests preview -- --test-threads=1` 绿。
5. **文档同步**：无。
6. **验证**：`rg "script-src 'unsafe-inline'" src-tauri/src/preview.rs` ≥ 2 命中（HOST_PAGE + 断言）；L1 定向绿。

## SEC-04（来源：review-02 R2-SEC-04）PreviewFrame sync 失败注释/文案失实

1. **位置**：`src/panels/docViewer/PreviewFrame.tsx:236-246`
2. **现状**：catch 注释 `// SEC-03：轮询下轮自愈，但静默吞错致链路故障零信号…` 与 warn 文案 `"[slTerminal] previewSync 失败（下轮轮询自愈，后续失败不再重复告警）:"`——失实：syncNow 等值早退（:223-225 几何五元组不变直接 return）使下轮轮询不重发 sync；真实重试源 = 几何变化 / 主窗移动（onMainWindowMoved）/ 面板重挂载。
3. **修复步骤**：:237-238 注释改 `// SEC-03：sync 失败不在等值轮询中重发（几何比较早退）——重试仅发生于几何变化/主窗移动/重挂载；静默吞错致链路故障零信号（SEC-01 前车之鉴）——首次失败告警一次，避免逐轮刷屏`；:242 文案改 `"[slTerminal] previewSync 失败（等值轮询不重发，几何变化/主窗移动时重试，后续失败不再重复告警）:"`。
4. **测试同步**：`rg "下轮轮询自愈" src/__tests__/html-panel.test.tsx` 自查——若现有 sync warn 用例断言含旧文案片段则同步改（实读确认点，执行 agent 必查）。
5. **文档同步**：无（docViewer/CLAUDE.md 无该文案引用）。
6. **验证**：`rg "下轮轮询自愈" src/` 零命中（含测试）；npm test 绿。

## BE-02（来源：review-03 R2-BE-02）notify 测试 shutdown 恢复 panic 检测

1. **位置**：`src-tauri/src/notify/mod.rs:933-981`（LoopHarness）
2. **现状**：shutdown()（:972-981）`assert!(join_with_timeout(self.handle, 5s))`——join_with_timeout 内部 `let _ = handle.join()`（thread_join.rs:24）吞 JoinError，watcher 线程 panic 时 `is_finished()=true` 即返回 true，断言恒过，panic 检测面丢失（原 `handle.join().unwrap()` 可检）。先例：settings.rs:518-552 线程结果经 mpsc 回传 + `rx.recv().unwrap().unwrap()`。
3. **修复步骤**（照 settings.rs mpsc 手法适配）：
   - LoopHarness 结构（:934-940）加字段 `done_rx: mpsc::Receiver<()>`（注释：`// 线程正常返回信令——panic 时发送端随线程死亡 drop，recv 必 Err（BE-02 panic 检测回传）`）；
   - start()（:943-968）：`let (done_tx, done_rx) = mpsc::channel::<()>();`；spawn 闭包改：

   ```rust
   move || {
       event_loop(&event_rx, &stop_rx, &paused_clone, &wps_clone, emitter.as_ref());
       let _ = done_tx.send(()); // 到达 = 未 panic；send 失败 = 接收端已弃，忽略
   }
   ```

   `Self { …, done_rx }` 一并构造；
   - shutdown() 末尾（assert 之后）追加：

   ```rust
   // panic 检测：event_loop 正常返回才发信——panic 退出则 done_tx 随线程死亡 drop，recv 必 Err
   self.done_rx.recv().expect("watcher 线程 panic——event_loop 未正常返回");
   ```

4. **测试同步**：harness 换装即全部 watcher_tests 经 shutdown 恢复 panic 检测；L1 定向 `cargo test --test lib_tests notify -- --test-threads=1` 全绿。负向传播语义 = mpsc drop→recv Err std 语义，不另造用例（本条目留痕该取舍）。
5. **文档同步**：notify/CLAUDE.md「测试模式」节补一句：LoopHarness shutdown = join_with_timeout + done 通道 panic 回传（线程结果信令，settings.rs mpsc 先例）。
6. **验证**：`rg "done_rx" src-tauri/src/notify/mod.rs` ≥ 3 命中（字段/构造/recv）；Read 确认 shutdown 含 recv expect 行；L1 定向绿。

## BE-06（来源：review-03 R2-BE-06）FileWatcher::drop 委托 stop()

1. **位置**：`src-tauri/src/notify/mod.rs:300-313`（Drop impl）对照 stop() :167-176
2. **现状**：`FileWatcher::drop` 逐行复制 stop 的信号发送 + join_with_timeout + warn 逻辑；`HookSignalWatcher::drop`（hooks/watcher.rs:146-150）已委托 `self.stop()`——两形态不一，未来改 stop 语义极易漏改 Drop 侧。
3. **修复步骤**：Drop impl 体整体替换为 `self.stop();`（stop 幂等：Option::take 双保险，重复调用安全）：

```rust
impl Drop for FileWatcher {
    fn drop(&mut self) {
        // 委托 stop()（幂等，Option::take 双保险）——与 hooks/watcher.rs 同形态（BE-06）
        self.stop();
    }
}
```

4. **测试同步**：L1 定向 `cargo test --test lib_tests notify -- --test-threads=1` 全绿（含 Drop 行为用例回归）。
5. **文档同步**：无。
6. **验证**：Read 确认 Drop 体仅 `self.stop()`；`rg -A3 "impl Drop for FileWatcher" src-tauri/src/notify/mod.rs` 无 join_with_timeout 复制段；L1 定向绿。

## BE-03（来源：review-03 R2-BE-03）游标解码失败三分支补测

1. **位置**：`src-tauri/src/fs/mod.rs:78-92`（decode_page_cursor）；测试落点 = 同文件内嵌 read_dir_tests（:786-1201 区）
2. **现状**：`let invalid = || AppError::Validation(format!("无效目录分页游标（opaque 契约）: {cursor}"));`（:80）三失败分支——base64 解码失败 / UTF-8 失败 / 无 NUL 或 tag 非 `D`|`F`——零用例；`rg "无效目录分页游标|decode_page_cursor" src-tauri/tests/` 零命中，现有分页用例只传合法游标或 None。base64 引擎 = `base64::engine::general_purpose::STANDARD`（:73/:81）。
3. **修复步骤**：read_dir_tests 内补 3 例（内嵌测试模块可直接调私有 decode_page_cursor；编码构造用同引擎）：
   - `decode_cursor_rejects_invalid_base64`：`decode_page_cursor("%%%!!!")` → `matches!(err, AppError::Validation(_))`；
   - `decode_cursor_rejects_invalid_utf8`：`STANDARD.encode([0xFF, 0xFE])` 产物传入 → Validation；
   - `decode_cursor_rejects_bad_tag_and_missing_nul`：`STANDARD.encode("X\u{0}x")`（坏 tag）与 `STANDARD.encode("Dx")`（无 NUL）两形态均 Validation。

   （用例函数名按「对象_行为_场景」约定微调可，不断言错误消息文案全等——Validation 变体匹配即可。）
4. **测试同步**：即本条；L1 定向 `cargo test --test lib_tests read_dir -- --test-threads=1` 绿（含新 3 例）。
5. **文档同步**：无（fs/CLAUDE.md 游标节已有边界登记，失败路径测试补齐不改口径）。
6. **验证**：`rg "decode_cursor_rejects" src-tauri/src/fs/mod.rs` 3 命中；L1 定向绿。

## BE-05（来源：review-03 R2-BE-05）spawn.rs 注释撞号消解

1. **位置**：`src-tauri/src/pty/spawn.rs:2155`
2. **现状**：`// ─── BE-06: join_with_timeout 用例已随迁 crate::thread_join::join_tests（BE-01）───`——「BE-06」指历史修复轮编号，与前轮计划 BE-06（scan_bench 基准两轮制）同号不同义；pty/CLAUDE.md 无该历史编号解码。
3. **修复步骤**：改 `// ─── 历史妥协修复轮注记: join_with_timeout 用例已随迁 crate::thread_join::join_tests（编号属该轮次，与本仓现行编号体系无关）───`。
4. **测试同步**：无（注释）。
5. **文档同步**：无。
6. **验证**：`rg "BE-06" src-tauri/src/pty/spawn.rs` 零命中；`cargo fmt --check` 绿。

## BE-01（来源：review-03 R2-BE-01）豁免表 join 计数收敛

1. **位置**：`.claude/test-exemptions.md:14`（CP-011 行）
2. **现状**：该行写「`join_with_timeout` 4 例」——BE-01 上轮去重随迁后实为 thread_join.rs `join_tests` 3 例（finished/blocked/abandoned）；同文件 :24（TQ-COV-03 行）已写 3 例，两行冲突。
3. **修复步骤**：:14 行「`join_with_timeout` 4 例」改「`join_with_timeout` 3 例」。
4. **测试同步**：无（文档计数）。
5. **文档同步**：即本条。
6. **验证**：`rg "join_with_timeout` 4 例" .claude/test-exemptions.md` 零命中；:14 与 :24 行均 3 例口径（Read 确认）。

## FE-02（来源：review-04 R2-FE-02）无快照分支双提交收口（先于 FE-01 执行）

1. **位置**：`src/features/explorer/useFileTree.ts:456-469`（restoreExpanded 无快照分支）+ :499-502 + :525-527
2. **现状**：无快照分支（:466-468）显式 `restoringRef.current = false; commitViewState();`。生产时序：loadRoot 内 `setRootNodes(首帧)`（:160）同步返回 → promise resolve → `.then(restoreExpanded)` 微任务先于 React 渲染宏任务执行——此时 commitViewState 读 rootNodesRef 过期空树上呼（第 1 次）；首帧渲染落定后 commit effect（:525-527）再触发（第 2 次）。同 payload 双上呼，与 checklist「恰好一次」契约相悖；L2 act 同步 flush 结构性不可暴露。
3. **修复步骤**：:466-468 删 `commitViewState()` 调用（保留 `restoringRef.current = false;`）——提交统一由首帧渲染落定后的 commit effect 承担（`setRootNodes(toTreeNodes(...))` 恒产新数组引用，空目录亦触发重渲染，提交必达）。:458-460 注释同步改写：`// FE-01: 无可恢复快照 → 无恢复窗口——解除加载抑制；首帧真实态提交由 rootNodes 渲染落定后的 commit effect 统一承担（此处显式提交在微任务先于心智渲染时读过期空树 → 生产双提交，FE-02 收口）`。
4. **测试同步**：use-file-tree.test.ts FE-01-2 用例（:1037-1040）随 FE-01 加严（见下条）；L2 定向 `npx vitest run use-file-tree` 绿。
5. **文档同步**：src/features/explorer/CLAUDE.md「提交时机」句尾补：「无快照分支不显式 commitViewState——微任务先于心智渲染读不到首帧树会双提交（FE-02），统一由渲染落定 commit effect 承担」。
6. **验证**：`rg -A2 "restoringRef.current = false" src/features/explorer/useFileTree.ts` 各解除点逐一 Read 确认：无快照分支/首帧失败 catch 无 commitViewState 随行；队列耗尽分支（:537-539）保留 commitViewState（恢复窗口最终提交兜底，语义不同不动）；npm test 绿。

## FE-01（来源：review-04 R2-FE-01）「恰好一次」断言锁死（FE-02 后执行）

1. **位置**：`src/__tests__/use-file-tree.test.ts:1037-1040`（FE-01-2 用例）
2. **现状**：`await waitFor(() => { expect(onViewStateChange).toHaveBeenCalled(); }, …)` + lastPayload 空集断言——≥1 语义，双提交实现也能绿。
3. **修复步骤**：waitFor 断言改 `expect(onViewStateChange).toHaveBeenCalledTimes(1);`（waitFor 轮询下双提交实现首轮即 2 次、恒红至超时——语义锁死）。
4. **测试同步**：即本条；`npx vitest run use-file-tree` 绿（依赖 FE-02 先修，否则必红——执行顺序已写死）。
5. **文档同步**：无。
6. **验证**：`rg "toHaveBeenCalledTimes\(1\)" src/__tests__/use-file-tree.test.ts` ≥ 1 命中（FE-01-2 用例内，Read 确认）。

## FE-03（来源：review-04 R2-FE-03）restoringRef 抑制超时兜底（用户裁决：修代码）

1. **位置**：`src/features/explorer/useFileTree.ts:479-518`（rootPath effect）
2. **现状**：:498 `restoringRef.current = true` 置位后，全部解除点（无快照分支/队列耗尽/首帧失败 catch/本 effect 下次运行）均依赖 loadRoot 的 promise settle——readDirPage 裸 invoke 无超时，Tauri 命令挂起场景加载窗口永不闭合，展开/折叠上呼被永久短路。
3. **修复步骤**：
   - :498 置位行后追加兜底定时器：

   ```ts
   // FE-03: 加载抑制超时兜底——loadRoot 永不 settle（后端挂起）时抑制永久悬挂；
   // 10s 后按 gen 校验兜底解除并上呼当前真实态（优于展开态槽位永久停摆）
   const suppressGuard = window.setTimeout(() => {
     if (gen === genRef.current && restoringRef.current) {
       restoringRef.current = false;
       console.warn("[slTerminal] loadRoot 超时未 settle，解除提交抑制兜底:", rootPath);
       commitViewState();
     }
   }, 10_000);
   ```

   - effect 末尾加 cleanup `return () => window.clearTimeout(suppressGuard);`（现有 effect 无 cleanup；rootPath 变化/effect 重跑时旧定时器作废）；
   - deps 加 `commitViewState`（`[rootPath, loadRoot, restoreExpanded, commitViewState]`，eslint exhaustive-deps）。
4. **测试同步**：use-file-tree.test.ts 补例 `FE-03: loadRoot 永不 settle → 10s 兜底解除抑制并上呼`——`vi.useFakeTimers()` + readDirPage 挂起桩 + `advanceTimersByTime(10_000)` → 断言 onViewStateChange 已上呼 + console.warn 一次；用例尾 `vi.useRealTimers()`。
5. **文档同步**：explorer/CLAUDE.md「提交时机」句尾补：「抑制兜底 = 10s 超时（FE-03）——loadRoot 永不 settle 时按 gen 校验解除并上呼当前真实态」。
6. **验证**：`rg "suppressGuard" src/features/explorer/useFileTree.ts` ≥ 2 命中（创建/cleanup）；`npx vitest run use-file-tree` 绿（含新例）。

## FE-04（来源：review-04 R2-FE-04）注释补第 4 解除点

1. **位置**：`src/features/explorer/useFileTree.ts:496-497`
2. **现状**：注释写「解除点：restoreExpanded 无快照分支 / 恢复队列耗尽 effect / 本 effect 下次运行」共 3 个——漏 loadRoot catch 首帧失败分支（:182-184 `restoringRef.current = false`，FE-07 联动）这第 4 个。
3. **修复步骤**：注释改「解除点：restoreExpanded 无快照分支 / 恢复队列耗尽 effect / loadRoot 首帧失败 catch / 本 effect 下次运行 / FE-03 超时兜底」（FE-03 落地后为 5 点，一并写全）。
4. **测试同步**：无（注释；解除点行为已由 FE-01-3 与 FE-03 用例锁）。
5. **文档同步**：无。
6. **验证**：Read 确认注释列举含「loadRoot 首帧失败 catch」与「超时兜底」。

## FE-07（来源：review-05 R2-FE-01）blockCache inflight 失效一并清除

1. **位置**：`src/panels/editor/largeFileViewer/blockCache.ts:95-103`（invalidateFile）+ :46-47（inflight 直返）
2. **现状**：invalidateFile 只清 cache + 推进代际，`inflight` 保留——失效 + setFileRev 复位后 useLineIndex 重建工作区重扫，首个 readBlock 命中仍在途旧任务（:46-47 直返无 gen 复核），旧文本被新扫描消费构建行索引；事件处理器已先把基线更新为新 meta，此后 mtime/size 未变事件不再二次失效 → 陈旧索引滞留。:58 gen 比对只门控缓存回填，没保住消费端。
3. **修复步骤**：invalidateFile 追加 inflight 前缀清除 + :95-96 头注更新：

```ts
/** 失效指定文件的全部缓存块 + 在途条目 + 推进代际（FE-05/FE-07）；在途调用方仍收响应
 * （Promise 对象存活），但失效后新扫描不再复用旧代际在途结果（防陈旧文本进新行索引）；
 * 旧任务 finally 的 inflight.delete 幂等无害 */
export function invalidateFile(filePath: string): void {
  fileGen.set(filePath, (fileGen.get(filePath) ?? 0) + 1);
  const prefix = `${filePath}${KEY_SEP}`;
  for (const k of [...cache.keys()]) {
    if (k.startsWith(prefix)) cache.delete(k);
  }
  for (const k of [...inflight.keys()]) {
    if (k.startsWith(prefix)) inflight.delete(k);
  }
}
```

4. **测试同步**：large-file-viewer.test.tsx「文件变更失效（FE-05）」describe（:221 起）补例 `invalidateFile 后新扫描不复用在途旧任务（重发 IPC 取新文本）`——mock readFileRange 第 1 次挂起/第 2 次返新文本：readBlock(FILE,0) 在途 → invalidateFile(FILE) → 再 readBlock(FILE,0) → 断言 readFileRange 被调 2 次且第 2 次调用方收到新文本（区别于 :239 既有例「旧代际不回填缓存」只断言缓存面）。
5. **文档同步**：editor/CLAUDE.md 大文件节「外部修改失效」口径随 FE-09/FE-10 一并更新（S05 内同 agent，不重复列）。
6. **验证**：`rg -A8 "export function invalidateFile" src/panels/editor/largeFileViewer/blockCache.ts` Read 确认含 inflight 清除段；`npx vitest run large-file-viewer` 绿（含新例）。

## FE-09（来源：review-05 R2-FE-03）首挂 stat 基线竞态封闭（先于 FE-10）

1. **位置**：`src/panels/editor/largeFileViewer/useLineIndex.ts:339-344`（return）+ `LargeFileViewer.tsx:81-97`（首挂 stat effect）+ :102-125（事件 effect）
2. **现状**：索引扫描首渲染即开始（getLine 探针），首挂 stat effect 后于其完成。交错窗口：扫描读到修改前字节 → 修改发生 → Modify 事件到达时 baseMetaRef 仍 null 被 :113 跳过 → stat resolve 于修改之后，基线直接 = 新 meta，此后同 mtime/size 事件全判未变，陈旧索引无再失效触发点。useLineIndex 返回 `{ lineCount, getLine, fullyIndexed, fatalError }`（:339-344），无索引进度信号（工作区 nextBlock 在 ref 内，fatalError 即渲染期直读 wsRef 先例）。
3. **修复步骤**：
   - useLineIndex return 加 `scannedBlocks: wsRef.current.nextBlock`（渲染期直读 ref，同 fatalError 先例；消费方仅 LargeFileViewer）；
   - LargeFileViewer：解构加 `scannedBlocks`；新增 `const scannedBlocksRef = useRef(0); scannedBlocksRef.current = scannedBlocks;`（渲染期同步 ref 模式先例）；
   - 首挂 stat effect then 内（:88 基线回填后）追加：

   ```ts
   // FE-09: 首挂基线竞态封闭——stat resolve 前索引已推进（可能扫到修改前字节）则
   // 保守失效重扫一次（成本一次重扫，换「base null 事件跳过 + 基线后设」窗口封闭；
   // fileRev+1 复位后 scannedBlocks 归零，stat effect deps 仅 [filePath] 不重跑，无循环）
   if (scannedBlocksRef.current > 0) {
     invalidateFile(filePath);
     setFileRev((r) => r + 1);
   }
   ```

   - :112 注释「基线未就绪…下次事件或重挂基线兜底」修正：`// 基线未就绪（首挂 stat 未归）→ 跳过：同文件不重挂（effect deps [filePath]），竞态窗口由 FE-09 首挂 resolve 封闭`。
4. **测试同步**：large-file-viewer.test.tsx 补例 `首挂 stat resolve 前索引已推进 → 保守失效重扫一次（FE-09）`——statFile mock 延迟 resolve、readFileRange 先返一版文本使扫描推进（scannedBlocks>0）→ 释放 stat → 断言触发一次失效重扫（readFileRange 重新请求块 0）。
5. **文档同步**：editor/CLAUDE.md「外部修改失效」句补：「首挂基线竞态 = stat resolve 时索引已推进则保守失效重扫一次封闭（FE-09）」。
6. **验证**：`rg "scannedBlocks" src/panels/editor/largeFileViewer/useLineIndex.ts src/panels/editor/largeFileViewer/LargeFileViewer.tsx` 各命中；`npx vitest run large-file-viewer` 绿（含新例）。

## FE-10（来源：review-05 R2-FE-04）失效比对加抽样指纹（用户裁决：改比对实现）

1. **位置**：`src/panels/editor/largeFileViewer/LargeFileViewer.tsx:70-125`（baseMetaRef :74、首挂 effect :81-97、事件 effect :102-125）
2. **现状**：失效判定仅 `m.mtimeMs !== base.mtimeMs || m.sizeBytes !== base.sizeBytes`（:113）——定长原位改写且 mtime 粒度未变（FAT/exFAT 2s 粒度、同毫秒连写）假阴性漏检；editor 域先例为读全文判等（useCodeMirror.ts:573/:575），viewer 域无对应层。
3. **修复步骤**：
   - baseMetaRef 类型扩 `{ sizeBytes: number; mtimeMs: number | null; fingerprint: string }`；
   - 模块级新增取样函数：

   ```ts
   /** 抽样指纹（FE-10）：首/中/末三段各 4KB 文本拼接——同 size 同 mtime 原位改写
    * （FAT 2s 粒度/同毫秒连写）mtime+size 比对假阴性的兜底判据；三段文本直接相等比对。
    * 成本：每次 Modify 事件 ≤12KB 三小段 IPC（fs-event 200ms debounce 天然节流） */
   async function sampleFingerprint(filePath: string, sizeBytes: number): Promise<string> {
     const SPAN = 4096;
     const mid = Math.max(0, Math.floor(sizeBytes / 2) - SPAN / 2);
     const tailStart = Math.max(0, sizeBytes - SPAN);
     const [head, middle, tail] = await Promise.all([
       fs.readFileRange(filePath, 0, SPAN),
       fs.readFileRange(filePath, mid, SPAN),
       fs.readFileRange(filePath, tailStart, SPAN),
     ]);
     return `${head}\n${middle}\n${tail}`;
   }
   ```

   - 首挂 stat then：基线回填前 `const fp = await sampleFingerprint(filePath, m.sizeBytes);`（then 回调改 async），基线含 fingerprint；
   - 事件 effect then 内（FE-09 落地后的形态上）改写比对：

   ```ts
   const base = baseMetaRef.current;
   if (base === null) return; // 基线未就绪（FE-09 注释口径）
   if (m.mtimeMs !== base.mtimeMs || m.sizeBytes !== base.sizeBytes) {
     const fp = await sampleFingerprint(filePath, m.sizeBytes);
     baseMetaRef.current = { sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs, fingerprint: fp };
     setFileMeta({ sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs });
     invalidateFile(filePath);
     setFileRev((r) => r + 1);
     return;
   }
   // FE-10: mtime/size 未变 → 抽样指纹复核（同 size 同 mtime 原位改写假阴性兜底）
   const fp = await sampleFingerprint(filePath, m.sizeBytes);
   if (fp !== base.fingerprint) {
     baseMetaRef.current = { sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs, fingerprint: fp };
     invalidateFile(filePath);
     setFileRev((r) => r + 1);
   }
   ```

4. **测试同步**：large-file-viewer.test.tsx「文件变更失效（FE-05）」describe 补两例——`同 size 同 mtime 改写 → 指纹变 → 失效重扫（FE-10）`（mock readFileRange 按 offset 返回可控文本，改写后指纹段变）与 `mtime/size/指纹均未变 → 不失效（缓存保留零重读）`（既有 :284 例随指纹层适配：断言事件后无新增失效，注意既有例的 mock 需支持指纹取样调用——执行 agent 适配既有夹具）。
5. **文档同步**：editor/CLAUDE.md「外部修改失效」句改写：`外部修改失效：fs-event Modify + fs_stat 比对（mtime/size 变化 → 缓存整表失效 + 行索引复位重扫）；mtime/size 未变时抽样指纹复核（首/中/末三段各 4KB，FE-10——同 size 同 mtime 原位改写假阴性兜底），静默重载（只读语义）；残余已知边界 = 三段 4KB 取样窗外的原位改写且 mtime 未变（理论残留，接受）+ 事件丢失残余窗口（同 editor 域 fs-event 依赖）。首挂基线竞态由 FE-09 封闭。`
6. **验证**：`rg "sampleFingerprint" src/panels/editor/largeFileViewer/LargeFileViewer.tsx` ≥ 3 命中（定义/两调用点）；`npx vitest run large-file-viewer` 绿（含新两例 + 既有例适配）。

## FE-08（来源：review-05 R2-FE-02）外部修改重载补大小防线（S05 内 pipeline 后位）

1. **位置**：`src/panels/editor/useCodeMirror.ts:546-602`（applyExternalChangeRef.current）；调用点 :631（fs-event event 路径）与 :462-469（CP-029 recheck 路径）
2. **现状**：:560 `content = await fs.readFile(path)` 全量读盘无任何大小防线——打开时 5MB 文件被外部改大到 50MB 后一次 Modify 即全量载入建 EditorView；打开路径 FE-08 已有 stat 预检（:357-394）+ 读后复核（:389-393），重载面断口。
3. **修复步骤**（照 :357-394 打开路径先例形态）：
   - :558 `let content: string;` 后、`try { content = await fs.readFile(path); }` 前插入：

   ```ts
   // FE-08 重载面补齐：外部修改重载同走 stat 预检——>10MB 不全量读盘灌 CM，
   // 置 largeFile 信号引导只读浏览（与打开路径同语义）；stat 失败按重载失败处理
   try {
     const meta = await fs.statFile(path);
     if (meta.sizeBytes > MAX_FILE_SIZE_BYTES) {
       filePathRef.current = undefined; // 防误保存覆盖原文件（同打开路径 :364）
       setLargeFile({ filePath: path });
       return;
     }
   } catch (err) {
     const msg = getErrorMessage(err);
     console.warn("[slTerminal] 外部修改重载失败:", msg);
     if (opts.toastOnError) toast.show("error", `外部修改重载失败: ${msg}`);
     return;
   }
   ```

   - 读盘后（:568 catch 块之后）补 TOCTOU 复核：

   ```ts
   // 读后复核（TOCTOU 防线，同打开路径 :389-393）：stat 与读盘间文件长大超限 → 仍引导只读浏览
   if (content.length > MAX_FILE_SIZE_BYTES) {
     filePathRef.current = undefined;
     setLargeFile({ filePath: path });
     return;
   }
   ```

   - 分支顺序说明（防执行 agent 重排）：事件路径脏分支确认弹窗（:549-557）先于 stat 预检——用户取消则不读 stat，顺序不动。
4. **测试同步**：use-code-mirror-reload-error.test.ts 补例 `外部修改重载超限 → largeFile 引导且 readFile 未调用（FE-08 重载面）`——mockStatFile 返 `sizeBytes: 11_000_000` → 触发 fs-event Modify → 断言 largeFile 信号置位（hook 返回值）+ mockReadFile 未被调；另补读后复核例（stat 返小、readFile 返超长串 → 同引导）。既有 mockStatFile 设施已备（该文件 hoisted mock 内含）。
5. **文档同步**：editor/CLAUDE.md 大文件节「读后复核保留为 TOCTOU 防线」句后补：「外部修改重载路径（applyExternalChange，event/recheck 双源）同走 stat 预检 + 读后复核 + largeFile 引导（FE-08 重载面补齐）」。
6. **验证**：`rg -n "setLargeFile" src/panels/editor/useCodeMirror.ts` ≥ 4 命中（打开路径 2 + 重载路径 2）；`npx vitest run use-code-mirror` 绿（含新例）；npm test 绿。

## SEC-02（来源：review-02 R2-SEC-02）L4 双 spec 未完整执行注记（历史档）

1. **位置**：`docs/compromises-fix-review-fix/checklist.md:102`（SEC-02 验证节 L4 双 spec 行）
2. **现状**：写死 `node e2e-tests/run-wdio.cjs --spec html.e2e.ts + --spec markdown.e2e.ts 全绿`——实际仅 html.e2e.ts 单 spec 执行（execution-plan 进度表 S02 行自证），markdown.e2e.ts 由 S06 全量 e2e（15 spec）间接覆盖，偏差未登记。
3. **修复步骤**：该行下补注记：`> 落地复核注记（review2-fix SEC-02，2026-09-11）：本轮 L4 实际仅执行 --spec html.e2e.ts；markdown.e2e.ts 未单独执行，由 S06 全量 e2e（15/15 exit 0，857dca0）间接覆盖。偏差登记，原文保留不改写。`
4. **测试同步**：无。
5. **文档同步**：即本条。
6. **验证**：`rg "落地复核注记（review2-fix SEC-02" docs/compromises-fix-review-fix/checklist.md` 1 命中。

## SEC-03（来源：review-02 R2-SEC-03）catch 零命中断言注记更正（历史档）

1. **位置**：`docs/compromises-fix-review-fix/checklist.md:161`（SEC-03 验证节）
2. **现状**：`rg -n "catch\(\(\) =>" src/panels/docViewer/PreviewFrame.tsx` 零命中——字面不成立：PreviewFrame.tsx:182 `void previewRender(...).catch(() => { … })` 为空参 catch（体内有 console.warn 可观测）存量保留形态，断言实际命中 1。留痕：review 报告另指 verify/stage-02.md:13 有同款断言——实读该文件全文无此断言（:13 为 syncWarned 断言），仅 checklist 一处。
3. **修复步骤**：该行下补注记：`> 落地复核更正（review2-fix SEC-03，2026-09-11）：「catch(() => 零命中」字面不成立——:182 previewRender 的 .catch(() => {…})（空参但体内含 console.warn 的可观测形态）为存量保留，实际命中 1；断言意图（零输出的静默吞错形态绝迹）已达。原文保留不改写。`
4. **测试同步**：无。
5. **文档同步**：即本条。
6. **验证**：`rg "落地复核更正（review2-fix SEC-03" docs/compromises-fix-review-fix/checklist.md` 1 命中。

## BE-04（来源：review-03 R2-BE-04）601 夹具偏差批注（历史档注记）

1. **位置**：`docs/compromises-fix-review-fix/checklist.md:295`（BE-02 步骤 6 写死「601 文件目录拉首页（500）」）
2. **现状**：实现改 1201 文件三页链（fs/mod.rs:1028 `seed_files(dir.path(), 1201)`）；commit 89988d6 body 已留痕理由（601 时第二页即末页，无法验续页游标链跨页传导）。
3. **修复步骤**：:295 行下补一行批注：`> 落地复核批注（review2-fix BE-04，2026-09-11）：实际执行为 1201 文件三页链（601 时第二页即末页，续页游标链无法验证），测试强度更高；commit body 已留痕。原文保留不改写。`
4. **测试同步**：无。
5. **文档同步**：即本条。
6. **验证**：`rg "落地复核批注（review2-fix BE-04" docs/compromises-fix-review-fix/checklist.md` 1 命中。

## FE-05（来源：review-04 R2-FE-05）stages/verify 路径错误注记更正（历史档）

1. **位置**：`docs/compromises-fix-review-fix/stages.md:169` + `docs/compromises-fix-review-fix/workflows/verify/stage-04.md:11`
2. **现状**：两处写 FE-02 目标文件 `src/panels/panelRegistry.ts`——实际为 `src/panelRegistry.ts`（无 src/panels/ 前缀目录，仓内 stat 实证不存在该路径）；按字面执行 rg 直接报错。
3. **修复步骤**：两处各在所属行/条目下补注记（原文保留）：`> 落地复核更正（review2-fix FE-05，2026-09-11）：FE-02 实际路径为 src/panelRegistry.ts（无 src/panels/ 前缀），原文保留不改写；按正确路径实跑 3 命中通过（review-04 实证）。`
4. **测试同步**：无。
5. **文档同步**：即本条。
6. **验证**：`rg "落地复核更正（review2-fix FE-05" docs/compromises-fix-review-fix/stages.md docs/compromises-fix-review-fix/workflows/verify/stage-04.md` 各 1 命中。

## DOC-03（来源：review-07 R2-DOC-03）execution-plan 计数矛盾注记（历史档）

1. **位置**：`docs/compromises-fix-review-fix/execution-plan.md:84`（row 04 备注「残留 8 处」/列举 10 点位）与 :87（row 07 备注「修正 11 处」）
2. **现状**：row 04 写「残留 8 处」但同格列举 10 个点位（injectScript.ts:11、PreviewFrame.tsx:13、assets.ts:12、markdown/CLAUDE.md:28/29/52、csp-config.test.ts:4/93/110、html-panel.test.tsx:362）；row 07 写「全仓修正 11 处」——且首轮修正漏 5 处现役注释（markdown-assets.test.ts:19、html.e2e.ts:163/293、markdown.e2e.ts:14/234，本轮 DOC-01 补齐）。
3. **修复步骤**：row 07 行下补注记：`> 落地复核更正（review2-fix DOC-03，2026-09-11）：计数口径不一——row 04 写 8 处而列举实为 10 点位，row 07 写 11 处；且首轮漏改 5 处现役注释（review2 DOC-01），由 review2-fix DOC-01 补齐。以实修点位为准，原文保留不改写。`
4. **测试同步**：无。
5. **文档同步**：即本条。
6. **验证**：`rg "落地复核更正（review2-fix DOC-03" docs/compromises-fix-review-fix/execution-plan.md` 1 命中。

## DOC-04（来源：review-07 R2-DOC-04）.temp/node22 断言恒假注记（历史档）

1. **位置**：`docs/compromises-fix-review-fix/workflows/verify/stage-07.md:9`（DOC-02 行）
2. **现状**：断言含「`.temp/node22` 预置通道仍在（Test-Path 确认未误删存活功能）」——run-wdio.cjs:322-341 实读确认按需预置机制（存在且 >1MB 才启用），常态目录不存在，断言恒假。
3. **修复步骤**：该行下补注记：`> 落地复核更正（review2-fix DOC-04，2026-09-11）：「.temp/node22 预置通道仍在」断言与常态不符——该目录为按需预置（run-wdio.cjs:322-341：存在且 >1MB 才启用），常态不存在，断言恒假；正确口径 = 预置逻辑代码仍在（rg 命中）。原文保留不改写。`
4. **测试同步**：无。
5. **文档同步**：即本条。
6. **验证**：`rg "落地复核更正（review2-fix DOC-04" docs/compromises-fix-review-fix/workflows/verify/stage-07.md` 1 命中。

## DOC-01（来源：review-07 R2-DOC-01）「无 CSP」失实表述 5 处修正

1. **位置**：`src/__tests__/markdown-assets.test.ts:19`、`e2e-tests/html.e2e.ts:163`、`:293`、`e2e-tests/markdown.e2e.ts:14`、`:234`（5 处均为注释，零逻辑）
2. **现状**（实读逐处）：:19 `// svg 载体可嵌脚本，预览域（无 CSP）内联风险面大——白名单剔除后本地`；html.e2e.ts:163 `宿主页域（自定义协议）无全局 CSP，fixture 内联事件属性/脚本真实执行；`；:293 `// （自定义协议宿主页 iframe，无全局 CSP）——内联 <script> 真实可执行。`；markdown.e2e.ts:14 `* 无 CSP 下同样成立；本地缺失图片必然尝试加载 → error 稳定触发；`；:234 `// raw HTML 事件属性通道（预览域无 CSP——onerror 正常执行）：`。SEC-02 后宿主页 HOST_PAGE 已由 meta 承载域级 CSP（default-src 'none'; script-src/style-src 'unsafe-inline'; img/font data:）。
3. **修复步骤**（逐处替换）：
   - markdown-assets.test.ts:19 → `// svg 载体可嵌脚本，预览域 script-src 'unsafe-inline' 下内联 svg 风险面大——白名单剔除后本地`（剔除理由实为 unsafe-inline 放行，与「无 CSP」无关）；
   - html.e2e.ts:163 → `宿主页域（自定义协议）无全局 CSP（域级 CSP 由宿主页 meta 承载，SEC-02——内联 script/style 与 img/font data: 放行），fixture 内联事件属性/脚本真实执行；`；
   - html.e2e.ts:293 → `// （自定义协议宿主页 iframe，无全局 CSP——域级 meta CSP 放行内联 script）——内联 <script> 真实可执行。`；
   - markdown.e2e.ts:14 → `* 域级 meta CSP 放行内联 script/style 下同样成立；本地缺失图片必然尝试加载 → error 稳定触发；`；
   - markdown.e2e.ts:234 → `// raw HTML 事件属性通道（预览域 meta CSP 放行内联 script——onerror 正常执行）：`。
4. **测试同步**：无（注释）。
5. **文档同步**：无。
6. **验证**：`rg "预览域（无 CSP）|预览域无 CSP|无 CSP 下" src/__tests__/markdown-assets.test.ts e2e-tests/html.e2e.ts e2e-tests/markdown.e2e.ts` 零命中；`rg "无全局 CSP" e2e-tests/html.e2e.ts` 命中行均含 meta CSP 补充说明（Read 确认）。

## DOC-02（来源：review-07 R2-DOC-02）adr/exemptions 现在时表述注记

1. **位置**：`.claude/adr.md:446`、`:459`、`:505` + `.claude/test-exemptions.md:39`
2. **现状**（实读）：adr.md:446「在预览域（无 CSP）渲染」、:459「预览域（无 CSP）内联风险面大」、:505「预览域维持无 CSP（data: img/font 天然放行，无代码落点）」三处现在时表述无注记指引；现成先例 :461「落地复核注记（SEC-02，2026-09-09，原文保留不改写）」仅覆盖 :460 一条。test-exemptions.md:39（已销项行）兜底栏含「预览域无 CSP」。
3. **修复步骤**（行内注记追加，不打断段落结构；统一口径「SEC-02 起宿主页 meta 承载域级 CSP」）：
   - adr.md:446 行尾「预览域（无 CSP）渲染」→「预览域（当时无 CSP——SEC-02 起为宿主页 meta 域级 CSP，见下落地复核注记）渲染」；
   - adr.md:459 行尾「预览域（无 CSP）内联风险面大」→「预览域（当时无 CSP，同上注记）内联风险面大」；
   - adr.md:505「预览域维持无 CSP（data: img/font 天然放行，无代码落点）」→「预览域当时维持无 CSP（data: img/font 天然放行，无代码落点——SEC-02 起宿主页 meta 承载域级 CSP，见 ADR-0018 落地复核注记）」；
   - test-exemptions.md:39「预览域无 CSP」→「预览域无 CSP（SEC-02 起为宿主页 meta 域级 CSP，见 adr.md ADR-0018 落地复核注记）」。
4. **测试同步**：无。
5. **文档同步**：即本条。
6. **验证**：`rg "预览域（无 CSP）|预览域无 CSP|预览域维持无 CSP" .claude/adr.md .claude/test-exemptions.md` 零命中（四处均改注后）。

## DOC-05（来源：review-05 R2-BE-01）skill 收尾约定补「提交即登记本 Stage 行」

1. **位置**：`.claude/skills/systematic-changes-execute/SKILL.md` 5.6 主 agent 编排节（:51-56）
2. **现状**：无登记时机纪律——前轮实证 S04 由 S05 捎带登记、S05 由 S07 回补，错位两级（S01-S03 各自提交自登）。
3. **修复步骤**：5.6 节追加一条：
   `- **Stage commit 提交即登记本 Stage 行**：execution-plan.md 进度跟踪表本 Stage 行（状态/commit hash/结果摘要）在该 Stage commit 完成后、下一 Stage 启动前必须登记——登记更新可并入紧随的文档/收尾提交，禁止跨 Stage 捎带迟登记（实证：compromises-fix-review-fix S04 由 S05 捎带、S05 由 S07 回补，登记错位两级）`
4. **测试同步**：无。
5. **文档同步**：即本条。
6. **验证**：`rg "提交即登记本 Stage 行" .claude/skills/systematic-changes-execute/SKILL.md` 1 命中。

## TE-05（来源：review-06 R2-TE-03）workspace/CLAUDE.md 菜单工厂签名同步

1. **位置**：`src/workspace/CLAUDE.md:75`
2. **现状**：登记 `createTabMenuItems(getApi, pageId|null, onRenameRequest, projectRootPath?)`——tabChrome.tsx:247-255 实读第 4 参已改回调形态 `getProjectRootPath?: (pageId: string | null) => string | undefined`（S11 共享宿主改造回归修复：单宿主下右键目标页在工厂构建期未知，须在 action 内按解析页现取）。
3. **修复步骤**：:75 签名改 `createTabMenuItems(getApi, pageId|null, onRenameRequest, getProjectRootPath?: (pageId:string|null) => string|undefined)`，并补半句「第 4 参为回调（非静态值）：右键目标页在工厂构建期未知，action 内按解析页现取（S11 回归修复）」。
4. **测试同步**：无（文档）。
5. **文档同步**：即本条。
6. **验证**：`rg "getProjectRootPath" src/workspace/CLAUDE.md` 命中且签名与 tabChrome.tsx:254 逐字一致（Read 对照）。
