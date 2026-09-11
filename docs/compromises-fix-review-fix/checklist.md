# 妥协修复 Review 问题修复清单（真值源）

> 输入：`docs/compromises-fix-review/`（5 份分域报告 + summary.md，30 条去重问题 + 附记 + 界外观察 + 9 项人工验证清单）。
> 范围裁决（用户 2026-09-09）：30 条全修 + 界外观察全收；preview label 放宽 `:` 和 `/`；预览域落 CSP meta；CP-006 游标 keyset 根治；blockCache 实现失效机制；CP-007 基准加固 + 串行登记；TQ-E-10 探针首 worker fast-fail + 其余降级；页内分屏消亡登记接受、不恢复。
> 编号：模块前缀 + 序号（SEC=预览安全 / BE=后端 / FE=前端 / TE=测试与工具链 / DOC=文档登记）。Stage 划分按 ID 引用。不用 P0-P4——执行优先级由 Stage 依赖顺序表达。
> 合并留痕：review-02 #1 与 review-03 #1 为同一问题（双 agent 独立复核）→ 合并为 SEC-01；review-03 #1 补充面（catch 静默）独立成 SEC-03；review-02 界外两测试裸 join 并入 BE-01；review-01 界外 vitest exclude → TE-07；review-04 界外 10MB 后置检查 → FE-08；review-05 界外 writeFakePlanEnv 重复 → TE-06；review 附记 CP-007 基准负载敏感 → BE-06。
> 现状行号全部经 2026-09-09 计划期实读核验（review 报告行号有漂移处按实读修正并留痕，如 BE-04）。
> 六段式段标容许内嵌：「现状」可并入条目标题下正文叙述；「测试同步/文档同步」可为步骤内编号行或显式「无」；DOC 域纯文档项的「文档同步」= 修复步骤本体、「测试同步」= 无（纯文档/演练），不单列段标——六类信息逐项齐全为准。

---

## SEC 域（预览链路安全，3 项）

### SEC-01 · preview validate_label 放宽 `:` `/`（review-02 #1 + review-03 #1 合并）

生产 panelId 为页前缀协议形态 `{pageId}:{localId}`（src/lib/panelId.ts，openFile.ts:98-100 产出处），预览 label = `preview-<panelId>` 必含 `:`；而后端 `validate_label` 仅放行字母数字 + `_` `-` → **生产预览链路四命令（preview_sync/preview_close/preview_render/preview_pull）全部 Err(Validation) 静默拒绝**，e2e 因直注裸 id（html.e2e.ts:329 `"e2e-html-close-" + Date.now()`）掩盖。tauri label 合法字符集实证（tauri-runtime-2.11.3 src/window.rs:534）：alphanumeric + `-` `/` `:` `_`。

**位置**：`src-tauri/src/preview.rs:51-68`

**现状**：

```rust
/// label 合法字符集：仅 ASCII 字母数字 + 下划线/连字符（窗口 label 与驱动句柄共用）
fn validate_label(label: &str) -> Result<(), AppError> {
    if !label.starts_with(PREVIEW_LABEL_PREFIX) { ... }
    let rest = &label[PREVIEW_LABEL_PREFIX.len()..];
    if rest.is_empty()
        || rest.len() > 96
        || !rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(AppError::Validation(format!("非法预览窗口 label: {label}")));
    }
    Ok(())
}
```

**修复步骤**：

1. `preview.rs:51` 注释改写为：`/// label 合法字符集：ASCII 字母数字 + _ - : /（对齐 tauri 窗口 label 合法集 tauri-runtime-2.11.3 window.rs:534；panelId 页前缀协议形态 {pageId}:{localId} 必含 ":"，SEC-01）`
2. `:63` 字符判定改为：

```rust
.all(|b| {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b':' || b == b'/'
})
```

3. 测试同步：`preview.rs` `mod preview_tests`（:563-591）两处改写——
   - `label_accepts_legal_panel_ids`（:569-573）合法集追加两个形态：`"preview-page-1:html-2"`（页前缀协议真实形态）、`"preview-a/b"`（`/` 放行）；
   - `label_rejects_illegal_forms`（:577-591）拒绝集**移除** `"preview-a/b"`（现合法），保留 `"main"` / `"preview-"` / 超长 97 / `"preview-中文"` / `"preview-a b"`。
4. L4 防复发（页面板真实前缀形态覆盖）：`e2e-tests/html.e2e.ts` 在「关闭 HTML 面板 → 预览窗口销毁出列」用例（:321-355）所在 describe 内新增一条用例——面板经生产打开链路（`window.__dockviewApi.addPanel` 的 id 用**页前缀协议形态** `"page-1:html-e2e-" + Date.now()`，模拟 openFile.ts 真实产出），断言 `waitPreviewDocContains(panelId, "...")` 成功（预览链路四命令不再被 Validation 拒绝）。注：html.e2e.ts 既有用例的裸 id 直注形态保留（各自测不同面），新增用例头注登记 SEC-01 防复发语义。
5. 文档同步：`src-tauri/src/CLAUDE.md`「preview.rs」节 label 校验行（「须 preview- 前缀 + ASCII 字母数字/下划线/连字符、≤96 字符」）改写为「须 preview- 前缀 + ASCII 字母数字/下划线/连字符/冒号/斜杠（tauri label 合法集对齐；panelId 页前缀含冒号）、≤96 字符」。

**验证**：

- `cargo test --test lib_tests preview -- --test-threads=1` 绿（含两改写用例）；
- `rg "preview-a/b" src-tauri/src/preview.rs` 仅命中合法集用例；
- L4：`node e2e-tests/run-wdio.cjs --spec html.e2e.ts` 绿（新用例含页前缀 label 全链路过 validate_label）。

---

### SEC-02 · 预览宿主页落地域级 CSP meta（review-03 #2）

预览域（自定义协议宿主页 + srcdoc iframe）当前**零 CSP**——预览内容任意出网（img/字体/fetch 外联全通），比 CP-012/033 计划的「局部 CSP 放行 font-src data:」形态更宽，未登记。用户裁决：HOST_PAGE 加 CSP meta。srcdoc iframe 继承宿主 CSP（W3C 行为），宿主页 meta 即覆盖预览渲染面。

**位置**：`src-tauri/src/preview.rs:449-458`（HOST_PAGE 常量 head 段）

**现状**：

```rust
const HOST_PAGE: &str = r##"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>slTerminal preview</title>
<style>
```

**修复步骤**：

1. `<meta charset="utf-8">` 行后插入一行：

```html
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:">
```

   指令设计（写死，勿自行加宽）：`default-src 'none'` 关闭一切出网（connect/font/img 默认全断）；`script-src 'unsafe-inline'` 必须——宿主页桥脚本与注入产物均内联；`style-src 'unsafe-inline'` 必须——注入样式与宿主 `<style>` 内联；`img-src data:` = markdown 本地图片内联通道（ADR-0018）；`font-src data:` = KaTeX 内联字体（CP-033 B2 形态落地）。iframe srcdoc 继承本 CSP（W3C），即预览文档同受此约束。
2. HOST_PAGE 头注（:440-448 区）补一句：`/// - 域级 CSP 经 meta 承载（tauri 2.11 无 per-webview CSP 配置面）——指令表见上，加宽须复核 SEC 面`。
3. 测试同步：`preview_tests` 新增用例 `host_page_carries_domain_csp`——断言 `HOST_PAGE.contains("Content-Security-Policy")`、`HOST_PAGE.contains("default-src 'none'")`、`HOST_PAGE.contains("font-src data:")`、`HOST_PAGE.contains("img-src data:")`。
4. 文档同步（四处）：
   - `src-tauri/src/CLAUDE.md` preview.rs 节「响应不带全局 CSP」表述后补「域级 CSP 由宿主页 meta 承载（default-src 'none'；script/style 'unsafe-inline'；img/font data:）」；
   - `src/panels/docViewer/CLAUDE.md`「渲染架构」节「自定义协议响应不带全局 CSP」句补同上口径；
   - `src/panels/CLAUDE.md` 预览家族节「域内无 CSP」相关表述改写为「域级 CSP meta（default-src 'none' + 内联 script/style + data: img/font）」；
   - `.claude/adr.md` ADR-0019 补一句落地注记（meta 形态 + 指令表）。
5. compromises.md CP-012 注记联动修订归 DOC-11（Stage 07）。

**验证**：

- `cargo test --test lib_tests preview -- --test-threads=1` 绿（新用例）；
- L4：`node e2e-tests/run-wdio.cjs --spec html.e2e.ts` + `--spec markdown.e2e.ts` 全绿（预览渲染/缩放/字体链路在 CSP 下不回归——data: 字体与内联脚本均在放行表内）。

> 落地复核注记（review2-fix SEC-02，2026-09-11）：本轮 L4 实际仅执行 --spec html.e2e.ts；markdown.e2e.ts 未单独执行，由 S06 全量 e2e（15/15 exit 0，857dca0）间接覆盖。偏差登记，原文保留不改写。

---

### SEC-03 · PreviewFrame sync/close catch 静默吞错可观测化（review-03 #1 补充面）

**位置**：`src/panels/docViewer/PreviewFrame.tsx:224-226`（sync catch）、`:243-245`（close catch）

**现状**：

```ts
void previewSync(label, x, y, rw, rh, visible, token).catch(() => {
  /* 窗口域异常——轮询下轮自愈 */
});
```

```ts
void previewClose(label, token).catch(() => {
  /* 窗口已不存在——幂等 */
});
```

SEC-01 修复前生产预览链路四命令全被拒，正因这两处静默 catch 零信号。

**修复步骤**：

1. sync catch（:224-226）：sync 由 200ms 轮询高频驱动（GEOMETRY_POLL_MS），逐次 warn 会刷屏——用一次性旗标（首次失败 warn、下次成功复位）：

```ts
// 组件作用域（label effect 内）加旗标（effect 重跑即重置）
let syncWarned = false;
// :224 catch 改为：
void previewSync(label, x, y, rw, rh, visible, token).catch((err) => {
  if (!syncWarned) {
    syncWarned = true;
    console.warn("[slTerminal] previewSync 失败（下轮轮询自愈，后续失败不再重复告警）:", label, err);
  }
});
// syncNow 成功路径（previewSync resolve 后）复位旗标——:216 的等值早退分支不改；
// 在 previewSync(...).then(...) 或 catch 链前补：
//   .then(() => { syncWarned = false; })
```

   落地形态（合并写）：`void previewSync(label, x, y, rw, rh, visible, token).then(() => { syncWarned = false; }).catch((err) => { ... })`。
2. close catch（:243-245）：close 一次性调用非高频，直接 warn：

```ts
void previewClose(label, token).catch((err) => {
  console.warn("[slTerminal] previewClose 失败（窗口可能残留）:", label, err);
});
```

3. render 路径（:177-182）已有 console.warn，不动。
4. 测试同步：`src/__tests__/html-panel.test.tsx`（PreviewFrame 集成面在此）新增用例 `previewSync 失败一次性告警不重复`——previewSync mock 连续 reject 两轮 → console.warn spy 恰好 1 次；mock 转 resolve → 再 reject → 再 warn 1 次（复位语义）。`markdown-panel.test.tsx` 不加（同组件覆盖即可）。

**验证**：

- `npx vitest run src/__tests__/html-panel.test.tsx` 绿（新用例）；
- `rg -n "catch\(\(\) =>" src/panels/docViewer/PreviewFrame.tsx` 零命中（空参静默 catch 形态绝迹）。

> 落地复核更正（review2-fix SEC-03，2026-09-11）：「catch(() => 零命中」字面不成立——:182 previewRender 的 .catch(() => {…})（空参但体内含 console.warn 的可观测形态）为存量保留，实际命中 1；断言意图（零输出的静默吞错形态绝迹）已达。原文保留不改写。

---

## BE 域（后端，6 项）

### BE-01 · 裸 join 清零——join_with_timeout 上提 crate 顶层共享件（review-02 #3 + 界外两测试 join）

CP-011 销项声称「无裸 join 无界阻塞路径」，实际三处生产裸 join 残留：`notify/mod.rs:172`（stop）、`notify/mod.rs:305`（Drop）、`hooks/watcher.rs:137`（stop）——均 `let _ = handle.join();` 无界阻塞。界外观察两处测试裸 join：`notify/mod.rs:968`（`self.handle.join().unwrap()`）、`hooks/watcher.rs:448`（`let _ = handle.join();`）。join_with_timeout 现居 `pty/reader.rs:175`（`pub(crate)`），notify/hooks 直接引用违反硬约束 #2（模块不穿透）——须先上提 crate 顶层新模块（app_dir/home 同形态先例）。

**位置**：新建 `src-tauri/src/thread_join.rs`；`src-tauri/src/pty/reader.rs:164-187`（迁出源）；`src-tauri/src/pty/spawn.rs:11/1532-1655`；`src-tauri/src/state.rs:11/41`；`src-tauri/src/notify/mod.rs:166-174/298-308/965-969`；`src-tauri/src/hooks/watcher.rs:131-139/431-449`；`src-tauri/src/lib.rs`（mod 声明）

**现状**（reader.rs 定义，随迁原样）：

```rust
pub(crate) const KILL_JOIN_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) fn join_with_timeout(handle: std::thread::JoinHandle<()>, timeout: Duration) -> bool { /* is_finished 轮询 + 内部 join */ }
```

**修复步骤**：

1. 新建 `src-tauri/src/thread_join.rs`——头注：`//! 线程 join 超时共享件（BE-01，自 pty/reader.rs 上提，app_dir/home 顶层共享件同形态）——全部线程退出点统一带超时 join，禁止裸 join 无界阻塞（CP-011 口径扩展：生产+测试全域，守卫见 src-tauri/src/CLAUDE.md）`。内容 = `JOIN_TIMEOUT` 常量（自 KILL_JOIN_TIMEOUT 改名，值 3s 不变）、`KILL_JOIN_POLL_INTERVAL`（10ms，随迁）、`join_with_timeout`（签名/实现原样随迁）；`CleanupPlan` 枚举与 `plan_cleanup_after_join_timeout` 是 pty 专有语义，**留 pty/reader.rs 不动**。
2. `src-tauri/src/lib.rs`：模块声明区加 `mod thread_join;`（按字母序插入既有 mod 列）。
3. `pty/reader.rs`：删除 :164-187 的常量与函数定义，改 `pub(crate) use crate::thread_join::{join_with_timeout, JOIN_TIMEOUT as KILL_JOIN_TIMEOUT};`——**不**，直接改消费点引用、不留别名（未来最优，禁兼容过渡）：reader.rs 内部调用点改 `crate::thread_join::join_with_timeout` / `crate::thread_join::JOIN_TIMEOUT`；`KILL_JOIN_TIMEOUT` 名在 pty 域语义内可保留——决定：消费点统一改用新名 `JOIN_TIMEOUT`，pty/CLAUDE.md 相关注释同步改名。
4. `pty/spawn.rs:11` import 改 `use crate::thread_join::{join_with_timeout, JOIN_TIMEOUT};`（`plan_cleanup_after_join_timeout`/`CleanupPlan` 仍自 reader import）；`:1532-1655` 内 `KILL_JOIN_TIMEOUT` 引用换 `JOIN_TIMEOUT`；`:2156-2186` 的 join_with_timeout 测试组（4 用例）**删除**（随迁去重）。
5. `state.rs:11` import 改 `use crate::thread_join::{join_with_timeout, JOIN_TIMEOUT};`，`:41` 调用改名。
6. `notify/mod.rs` stop（:171-173）与 Drop（:304-306）换装：

```rust
if let Some(handle) = self.thread_handle.take() {
    if !crate::thread_join::join_with_timeout(handle, crate::thread_join::JOIN_TIMEOUT) {
        tracing::warn!("notify watcher 线程 3s 内未退出——detach 由进程退出回收（BE-01）");
    }
}
```

7. `hooks/watcher.rs` stop（:136-138）同款换装（warn 文案主语换「hook 信号 watcher」）。
8. 测试裸 join 两处换装：`notify/mod.rs:968` → `assert!(crate::thread_join::join_with_timeout(self.handle, std::time::Duration::from_secs(5)), "测试 watcher 线程 5s 内应退出");`；`hooks/watcher.rs:448` → 同款 assert（该处 :441-446 已轮询确认 finished，join_with_timeout 即时成功）。
9. `thread_join.rs` 测试模块 `mod join_tests`：自 reader.rs :882-907 与 spawn.rs :2156-2186 两组合并去重随迁（同语义用例只留一份）——`join_with_timeout_finished_handle_returns_true` / `join_with_timeout_blocked_thread_returns_false` / `join_with_timeout_abandoned_thread_finishes_later_no_panic`（超时后 detach 线程稍后可自行结束、不 panic）。
10. 文档同步：`src-tauri/src/CLAUDE.md`「存在理由」节顶层模块清单加一行 thread_join.rs（「线程 join 超时共享件（BE-01）」）；`pty/CLAUDE.md` 引用 `join_with_timeout`/`KILL_JOIN_TIMEOUT` 处改新名与新位置。
11. compromises.md CP-011 注记与 compromises-fix/checklist.md 验证节命令回写归 DOC-06（Stage 07）。

**验证**：

- `rg "\.join\(\)" src-tauri/src` 仅命中 `thread_join.rs` 内一处（守卫白名单）；
- `rg "KILL_JOIN_TIMEOUT" src-tauri/src` 零命中（全改名 JOIN_TIMEOUT）；
- `rg "pty::reader::\{[^}]*join_with_timeout" src-tauri/src` 零命中（无跨模块引用）；
- `cargo test --test lib_tests -- --test-threads=1` 全绿（含 thread_join 随迁用例与 notify/watcher 换装后生命周期用例）。

---

### BE-02 · fs_read_dir 游标改 keyset（目录中途变长跨页重复/遗漏根治，review-02 #5）

用户裁决「keyset 根治」。现状：游标 = base64（下一页起始序号）——目录在分页间变长（新增条目排序在游标前）时序号平移 → 跨页重复/遗漏。改 keyset：游标 = 上一页末条目排序键（isDir + 小写名），续页取严格大于该键的后缀——天然无重复无遗漏。前端 opaque 契约零改动。

**位置**：`src-tauri/src/fs/mod.rs:55-68`（encode/decode_page_cursor）、`:499-501`（契约注释）、`:512-588`（fs_read_dir_impl 切片段）

**现状**：

```rust
fn encode_page_cursor(start: usize) -> String { /* base64(offset 十进制) */ }
fn decode_page_cursor(cursor: &str) -> Result<usize, AppError> { ... }
```

```rust
// :512-520 区
let start = cursor
    .as_deref()
    .map(decode_page_cursor)
    .transpose()?
    .unwrap_or(0);
// :566-570 排序：b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(...))
// :573-578 切片 entries[start..end]；:580 next_cursor = (end < len).then(|| encode_page_cursor(end))
```

**修复步骤**：

1. 排序键函数（:55 前新增）：

```rust
/// 排序键（keyset 游标比较基准）——与排序契约同源：目录在前、同类型小写名称序
/// （b.is_dir.cmp(&a.is_dir) 降序 = 目录(true) 排前 → 键序 (0=目录, 1=文件)）
fn sort_key(e: &DirEntry) -> (u8, String) {
    (if e.is_dir { 0 } else { 1 }, e.name.to_lowercase())
}
```

2. 游标编解码改写（:55-68 整段替换）：

```rust
/// keyset 游标编码：base64("D\0"<小写名>) / base64("F\0"<小写名>)——上一页末条目排序键；
/// NUL 不出现在任何文件名中，分隔安全。opaque 契约不变（客户端只回传不解读）
fn encode_page_cursor(is_dir: bool, name: &str) -> String {
    use base64::Engine;
    let tag = if is_dir { "D" } else { "F" };
    base64::engine::general_purpose::STANDARD
        .encode(format!("{tag}\u{0}{}", name.to_lowercase()))
}

fn decode_page_cursor(cursor: &str) -> Result<(u8, String), AppError> {
    use base64::Engine;
    let invalid = || AppError::Validation(format!("无效目录分页游标（opaque 契约）: {cursor}"));
    let raw = base64::engine::general_purpose::STANDARD
        .decode(cursor)
        .map_err(|_| invalid())?;
    let text = String::from_utf8(raw).map_err(|_| invalid())?;
    let (tag, name) = text.split_once('\u{0}').ok_or_else(invalid)?;
    let dir_key = match tag {
        "D" => 0u8,
        "F" => 1u8,
        _ => return Err(invalid()),
    };
    Ok((dir_key, name.to_string()))
}
```

   （base64 crate 依赖现状沿用——:55-68 现有实现用什么写法就保留什么写法，上块仅示形态，执行时与现有 import 风格对齐。）
3. `fs_read_dir_impl`（:512-588）切片段改造：排序不变；起始索引改 keyset 定位：

```rust
let start = match cursor.as_deref() {
    Some(c) => {
        let key = decode_page_cursor(c)?;
        // 严格大于游标键的第一个条目（目录中途变长、新增条目排序在游标前 → 不重复不遗漏）
        entries.partition_point(|e| sort_key(e) <= key)
    }
    None => 0,
};
```

   `next_cursor`（:580）改：`let next_cursor = (end < entries.len()).then(|| { let last = &entries[end - 1]; encode_page_cursor(last.is_dir, &last.name) });`
4. 契约注释（:499-501）改写：游标 = keyset（上一页末条目排序键 base64），越界语义 = 游标键大于现存全部条目 → 空页 + null（形态保留）。
5. 边界登记（fs/CLAUDE.md 契约节同步改写时写入）：排序键 `(isDir, 小写名)` 冲突（大小写变体同名）时 keyset 续页对同键后缀条目可能跳过/重复——Windows 默认大小写不敏感下该形态同目录不可创建，登记为理论边界（大小写敏感目录标志边缘场景）。
6. 测试同步（fs/mod.rs 内嵌测试组）：
   - 现有分页用例（`read_dir_first_page_has_cursor_when_overflow` / `read_dir_cursor_resume_mid_list` / `read_dir_page_limit_clamped_to_max` / `read_dir_sort_order_stable_across_pages` / `read_dir_git_filter_still_applied`）不解读游标内容，预期零改动全绿——若有个别断言游标形态（如解 base64 得数字），按 keyset 形态适配；
   - 新增 `read_dir_keyset_cursor_growth_no_dup_no_hole`：601 文件目录拉首页（500）→ 在排序于游标**前**的位置新增条目（如名为 `0aaa` 的目录/文件）→ 续页拉取 → 断言续页与首页拼接无重复、新增条目不出现（排序在游标前的新增属「已翻过的页」，正确语义）→ 再在排序于游标**后**的位置新增条目（`zzzz.txt`）→ 再续页 → 断言新增条目出现且无重复；

   > 落地复核批注（review2-fix BE-04，2026-09-11）：实际执行为 1201 文件三页链（601 时第二页即末页，续页游标链无法验证），测试强度更高；commit body 已留痕。原文保留不改写。

   - 新增 `read_dir_keyset_cursor_beyond_end_empty_page`：用末页游标（或手工构造大于全部条目的键编码）续拉 → 空页 + `next_cursor.is_none()`。
7. 文档同步：`src-tauri/src/fs/CLAUDE.md`「fs_read_dir 游标分页」节改写（keyset 口径 + 上述边界登记）；`src/ipc/CLAUDE.md`「目录分页读取（CP-006）」节同步一句（游标 keyset 化，opaque 契约不变，前端零改动）。
8. compromises.md CP-006 注记联动归 DOC-07（Stage 07）。

**验证**：

- `cargo test --test lib_tests fs_read_dir -- --test-threads=1` + `cargo test --test lib_tests read_dir -- --test-threads=1` 全绿（含两新增用例）；
- 前端零改动验证：`git diff --stat src/ipc/fs.ts src/features/explorer/useFileTree.ts` 为空（keyset 对 opaque 消费方透明）；
- L2 `npm test` 绿（use-file-tree 续页链路回归）。

---

### BE-03 · git/mod.rs:185 注释删 ignored 残留（review-02 #8）

**位置**：`src-tauri/src/git/mod.rs:185`

**现状**：注释「非 renamed（modified/added/untracked/conflict/ignored）」——ignored 为死分支清理残留（:53-55 自证 include_ignored 恒关永不置位，CP-008 已销项删死分支）。

**修复步骤**：`:185` 注释改为「非 renamed（modified/added/untracked/conflict）」。

**测试同步**：无（注释-only）；CP-008 防复发用例 `git_status_ignored_file_never_emitted` 已在位守护语义。

**文档同步**：无（git/CLAUDE.md 红线已是「不要恢复 include_ignored」口径）。

**验证**：`rg "ignored" src-tauri/src/git/mod.rs` 仅命中 CP-008 语义相关行（如测试/登记注释），:185 不再含 ignored 列举；`cargo test --test lib_tests git -- --test-threads=1` 绿。

---

### BE-04 · CP-005 grep 守卫自匹配修复（review-02 #6）

守卫意图：`rg "std::sync::(Mutex|RwLock)"` 应零命中。但守卫描述行自身含该字面量子串——按字面执行恒非零命中（自匹配）。实读核验：守卫描述行现位于 `src-tauri/src/CLAUDE.md:64`（review 报告称 :53，已漂移）——该行文本「禁止再引入 `std::sync::Mutex/RwLock`（grep 守卫）」含 `std::sync::Mutex` 子串。

**位置**：`src-tauri/src/CLAUDE.md:64`

**现状**：「新建持锁临界区一律 parking_lot，禁止再引入 `std::sync::Mutex/RwLock`（grep 守卫）。」

**修复步骤**：该行改写为：「新建持锁临界区一律 parking_lot，禁止再引入 std Mutex/RwLock 原语——grep 守卫命令写死为 `rg "std::sync::(Mutex|RwLock)" src-tauri/src -g "*.rs"`（-g 限定 Rust 源文件，本行文档描述不自匹配）。」

**测试同步**：无（文档/守卫命令形态）。

**文档同步**：本项即文档修复；`docs/compromises-fix/checklist.md` CP-005 验证节（:674）命令回写归 DOC-06（Stage 07）。

**验证**：`rg "std::sync::(Mutex|RwLock)" src-tauri/src -g "*.rs"` 零命中（退出码 1）；改写后该行文本不含可命中子串（`rg "std::sync::(Mutex|RwLock)" src-tauri/src/CLAUDE.md` 零命中）。

---

### BE-05 · 新增 fs_stat 命令（真实字节数/mtime 通道——FE-04/05/08 共同依赖）

**位置**：`src-tauri/src/fs/mod.rs`（impl + DTO）；`src-tauri/src/lib.rs:122-129`（generate_handler fs 段）；`src-tauri/build.rs:20-59`（commands 清单 42 条 + :20 计数注释）；`src-tauri/capabilities/default.json:27-34`（fs perms 段）；`src/ipc/fs.ts`（wrapper）；`src/types/fs.ts`（ts-rs 生成物，禁手改）

**跨边界契约（写死，两侧 agent 不各自推断）**：

- 命令名：`fs_stat`；参数：`{ path: string }`；
- 返回 DTO：`FsMetadata = { sizeBytes: number; mtimeMs: number | null }`（Rust：`size_bytes: u64`、`mtime_ms: Option<i64>`，serde camelCase + `#[derive(TS)]`，CP-024 单源——`src/types/fs.ts` 由 export_bindings 测试生成，禁手改）。

**修复步骤**：

1. `fs/mod.rs` 加 DTO：

```rust
/// 文件元数据（fs_stat）——真实字节数 + 修改时间（大文件信息条/失效比对基线）
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../src/types/fs.ts")]
#[serde(rename_all = "camelCase")]
pub struct FsMetadata {
    pub size_bytes: u64,
    /// 修改时间（Unix 毫秒）；文件系统不支持/早于 epoch → null
    pub mtime_ms: Option<i64>,
}
```

   （TS derive 的 ts 属性形态与现有 DirEntry/FsReadDirPage 完全一致——执行时照同文件 :25-45 先例对齐，勿自创。）
2. 加实现与命令（spawn_blocking + 沙箱校验同 fs_read_file 先例：目标路径经 `validate_path_within_root`）：

```rust
pub async fn fs_stat_impl(path: String, root: Option<PathBuf>) -> Result<FsMetadata, AppError> {
    // 沙箱校验（目标路径，与 fs_read_file 同款）→ spawn_blocking：
    let meta = std::fs::metadata(&validated)?; // io_error 辅助函数包装，error.rs 先例
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);
    Ok(FsMetadata { size_bytes: meta.len(), mtime_ms })
}
```

3. 三处注册：`lib.rs` generate_handler fs 段加 `fs::fs_stat`；`build.rs` commands 清单加 `"fs_stat"`（:20 计数注释 42→43）；`capabilities/default.json` fs perms 段加 `"allow-fs-stat"`。
4. `src/ipc/fs.ts` 加 wrapper：

```ts
/** 文件元数据（fs_stat）——真实字节数 + 修改时间；沙箱校验同 readFile */
export function statFile(path: string): Promise<FsMetadata> {
  return invoke<FsMetadata>("fs_stat", { path });
}
```

   （import type FsMetadata from ../types/fs。）
5. DTO 生成：`cargo test --test lib_tests export_bindings -- --test-threads=1` → `git diff --exit-code -- src/types` 守卫须绿（生成物含 FsMetadata）。
6. 测试同步：
   - L1（fs/mod.rs 内嵌测试组）：`fs_stat_returns_size_and_mtime`（tempfile 写已知内容 → size_bytes 精确相等 + mtime_ms 非 None）；`fs_stat_missing_file_errs`（不存在路径 → Err）；`fs_stat_outside_root_rejected`（沙箱外 → Err，照现有沙箱用例形态）；
   - L2：`src/__tests__/ipc-contract.test.ts`（或现有 fs 契约测试文件）按声明式 schema 补 fs_stat 条目（命令名 + payload 键集合精确断言 + 返回透传——照现有 fs 条目先例）。
7. 文档同步：`src-tauri/src/fs/CLAUDE.md` 加一行 fs_stat 段（命令语义 + 无 10MB 上限——stat 不读内容）；`src/ipc/CLAUDE.md` fs 段补 wrapper 一句。

**验证**：

- `cargo test --test lib_tests fs_stat -- --test-threads=1` 绿；
- `cargo test --test lib_tests export_bindings -- --test-threads=1` 后 `git diff --exit-code -- src/types` 绿；
- `npx tsc --noEmit` + L2 ipc 契约测试绿；
- `rg "fs_stat" src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json src/ipc/fs.ts` 四处全命中。

---

### BE-06 · CP-007 基准用例负载加固（review 附记：并行负载误红）

`scan_bench_1000_sessions_median_under_200ms`（scan.rs:843-879）在 L1/L2 并行抢 CPU 时中位实测 410ms 误红（隔离复跑 180ms 稳绿）。用户裁决「基准加固 + 串行登记」。

**位置**：`src-tauri/src/agent_history/claude/scan.rs:861-878`

**现状**：样本 20 次取 `samples[9]` 低中位，单次断言 `median < 200ms`（:875-878），max 仅报告（:871-874 注释已登记）。

**修复步骤**：

1. 断言改两轮制（瞬态负载隔离）：首次中位越门槛 → 冷却 2s → 重采样 20 次再判，两轮均越才红：

```rust
// 断言段（:869-878）替换为：
let median = samples[9]; // 20 样本取低中位(第 10 小)
let max = samples[19];
eprintln!("CP-007 基准(轮 1): 样本=20 中位={median:?} max={max:?}(每样本 = 1000 会话全扫)");
let threshold = Duration::from_millis(200);
if median >= threshold {
    // 负载加固（BE-06，2026-09-09）：并行抢 CPU 的瞬态负载曾致单轮中位 410ms 误红
    // （隔离复跑 180ms）——冷却 2s 重采样一轮再判，两轮均越门槛才红（真实回归两轮必越，
    // 瞬态尖峰第二轮自愈）。仍不抗持续并行负载——全量回归 L1/L2 串行纪律见根 CLAUDE.md。
    eprintln!("CP-007 基准(轮 1) 中位越门槛——冷却 2s 重采样复判");
    std::thread::sleep(Duration::from_secs(2));
    let mut samples2: Vec<Duration> = Vec::with_capacity(20);
    for _ in 0..20 {
        let t0 = Instant::now();
        let sessions = scan_sessions_with_force(true);
        samples2.push(t0.elapsed());
        assert_eq!(sessions.len(), 1000, "重采样每次样本应全量命中 1000 会话");
    }
    samples2.sort();
    let median2 = samples2[9];
    eprintln!("CP-007 基准(轮 2): 中位={median2:?} max={:?}", samples2[19]);
    assert!(
        median2 < threshold,
        "全扫中位两轮均越常驻门槛 200ms(轮1={median:?} 轮2={median2:?},依据 9-8 实测 180.33ms)——若回落 <50ms 重评估删除分支,放宽须留数字依据(CP-007)"
    );
}
```

2. 头注（:838-842 flaky 处置段）追加一句：「9-9 复核加固：L1/L2 并行抢核单轮中位 410ms 误红 → 断言改两轮制（冷却 2s 重采样复判）；持续并行负载仍不抗——L1/L2 串行回归纪律登记根 CLAUDE.md（BE-06）」。
3. 测试同步：本项即测试改造（benchmark 用例自体）；防复发 = 两轮制语义由用例注释承载（无独立单测——基准用例不适配确定性断言，属测试豁免语义，登记见下）。
4. 文档同步：agent_history/CLAUDE.md 无需动（基准语义不变）；串行纪律登记根 `.claude/CLAUDE.md` 归 DOC-09（Stage 07）。

**验证**：

- `cargo test --test lib_tests scan_bench -- --test-threads=1` 绿（串行环境单轮过，不触发重采样）；
- `rg -c "重采样" src-tauri/src/agent_history/claude/scan.rs` ≥ 3（注释 + 两轮代码路径）。

---

## FE 域（前端，11 项）

### FE-01 · useFileTree 挂载空提交抑制（CP-016 恢复窗口破口，review-04 #1）

commit effect（useFileTree.ts:501-503）在每次 rootNodes 渲染后调 commitViewState；挂载后首帧 loadRoot 完成前，初始空树渲染即触发一次「空集上呼」覆盖注册表状态槽——该窗口内卸载再切回则展开态丢失（restoreExpanded 读到空集）。短路机制已存在（commitViewState :417 `if (restoringRef.current) return;`），破口 = 加载窗口期 restoringRef 未置位（restoreExpanded 仅在有快照可恢复时才置位，:453）。

**位置**：`src/features/explorer/useFileTree.ts:444-455`（restoreExpanded）、`:457-478`（rootPath effect）

**现状**：

```ts
const restoreExpanded = useCallback(() => {
  const stored = viewStateRef.current;
  if (!stored) return;
  if (stored.rootPath !== rootPathRef.current) return;
  const paths = [...stored.expandedPaths].sort((a, b) => a.length - b.length);
  if (paths.length === 0) return;
  // 恢复窗口开启：抑制逐层提交（队列耗尽由消费 effect 解除并一次性上呼）
  restoringRef.current = true;
  setRestoreQueue(paths);
}, []);
```

rootPath effect（:472-477）rootPath 非 null 分支直接 `loadRoot(gen).then(() => { ...restoreExpanded(); })`，此前无抑制置位。

**修复步骤**：

1. rootPath effect 的 rootPath 非 null 分支（:472-474 清空三行之后、`loadRoot(gen)` 之前）加：

```ts
// FE-01: 加载窗口开启——loadRoot 完成前抑制 commitViewState 空提交覆盖注册表槽
// （解除点：restoreExpanded 无快照分支 / 恢复队列耗尽 effect / 本 effect 下次运行）
restoringRef.current = true;
```

2. restoreExpanded 三分支收口改写（:444-455 整段替换）：

```ts
const restoreExpanded = useCallback(() => {
  const stored = viewStateRef.current;
  // FE-01: 无可恢复快照（无存/域键不符/空展开集）→ 无恢复窗口——解除加载抑制
  // 并提交首帧真实态（此前空树渲染的 commit effect 已被 restoringRef 短路，
  // 此处补上呼保证槽位反映真实首帧）
  if (!stored || stored.rootPath !== rootPathRef.current || stored.expandedPaths.length === 0) {
    restoringRef.current = false;
    commitViewState();
    return;
  }
  const paths = [...stored.expandedPaths].sort((a, b) => a.length - b.length);
  // 恢复窗口开启：抑制逐层提交（队列耗尽由消费 effect 解除并一次性上呼）
  restoringRef.current = true;
  setRestoreQueue(paths);
}, [commitViewState]);
```

3. loadRoot 首帧失败路径联动（FE-07 的 catch 改造内一并解除抑制，见 FE-07 步骤代码块——两项同 agent 同函数落地）。
4. 测试同步（`src/__tests__/use-file-tree.test.ts`）：
   - 新增 `挂载加载窗口期不上呼空提交`：renderHook 挂载（viewState 有历史展开快照）→ readDirPage mock 未 resolve 期间断言 onViewStateChange spy 零调用；resolve 后（恢复快照命中路径）仍零调用直至队列耗尽。
   - 新增 `无快照场景首帧落地后提交一次真实态`：viewState=null 挂载 → loadRoot resolve → onViewStateChange 恰好一次且 expandedPaths 反映首帧（空集——但时机在首帧落地后，非挂载即空提交）。
   - 回归：`loadRoot 首帧失败后解除抑制`（与 FE-07 用例同组）：readDirPage 首帧 reject → restoringRef 解除（后续 toggleExpand 后 commit 正常上呼）。
5. 文档同步：`src/features/explorer/CLAUDE.md`「展开态跨挂载契约（CP-016）」节「提交时机」条补一句「挂载加载窗口期（loadRoot 完成前）提交经 restoringRef 抑制（FE-01）——空树渲染不再上呼覆盖槽位」。

**验证**：

- `npx vitest run src/__tests__/use-file-tree.test.ts` 绿（含三新增用例）；
- `rg -n "restoringRef.current = true" src/features/explorer/useFileTree.ts` ≥ 2（加载窗口 + 恢复窗口两置位点）。

---

### FE-02 · PANEL_SETTINGS 常量补齐（review-04 #2）

**位置**：`src/panelRegistry.ts:23`（常量区）、`:86`（PANEL_TYPES）、`:118`（isAlwaysRenderPanel）

**现状**：

```ts
export const PANEL_MARKDOWN_VIEWER = "markdownviewer" as const;
// PANEL_GIT_SHOW/PANEL_DIFF/PANEL_HOOKS_CONFIG 已删除（FE-35）——
// 全仓零外部消费（grep 无 import），内部 PANEL_TYPES/FILE_PANEL_TYPES 改字面量。
```

```ts
export const PANEL_TYPES = [
  PANEL_TERMINAL, PANEL_EDITOR, PANEL_HTML_VIEWER, PANEL_MARKDOWN_VIEWER,
  "gitshow", "diff", "settings",
] as const;
```

```ts
// isAlwaysRenderPanel :118
type === "settings"
```

**修复步骤**：

1. `:23` 后新增：

```ts
/** 设置中心面板类型标识 */
export const PANEL_SETTINGS = "settings" as const;
```

2. `:86` `"settings",` → `PANEL_SETTINGS,`；`:118` `type === "settings"` → `type === PANEL_SETTINGS`。
3. 边界收窄说明（写死）：组件映射表（:54-76）的键保持字面量形态不动（该对象五键均为字面量，键位字面量是 JS 对象惯用形态；review 点名面仅 :86/:118 两处与常量混用位）。pageApis.ts 的 `component: "settings"` 与 `panelIdInPage(pageId, "settings")` 是 localId/组件名语义槽，不换（不同语义层，防过度收敛）。
4. 测试同步：`src/__tests__/panel-registry.test.ts` 零改动（断言值字面量，值不变不破）。

**验证**：`npx vitest run src/__tests__/panel-registry.test.ts` 绿；`rg -n '"settings"' src/panelRegistry.ts` 仅命中 :24（常量定义值）与 :73（映射表键）。

---

### FE-03 · LargeFileViewer 行文本色响应式取色（review-04 #3）

**位置**：`src/panels/editor/largeFileViewer/LargeFileViewer.tsx:22-27`（模块级快照）、`:149`/`:178-194`（消费点）

**现状**：

```ts
const LINE_TEXT_COLOR = schemeRegistry.getActive().editor.overrides.plainText;
```

模块加载期求值一次，全生命周期冻结——CP-039 刚消灭 editorTheme 同类冻结面，此处重新引入。

**修复步骤**：

1. 删除 :22-27 模块级常量（注释块随迁语义进组件）；`LINE_FONT_FAMILY`（:29）保留模块级（EDITOR_FONT_SPEC 为静态规格，非方案色）。
2. 组件内改渲染期 state + 订阅：

```tsx
// FE-03: 行文本色 = active 方案 editor.overrides.plainText——渲染期取值 +
// onDidChange 订阅响应式更新（editorThemeSlot 先例照抄；模块级 import 期快照已废）
const [lineTextColor, setLineTextColor] = useState(
  () => schemeRegistry.getActive().editor.overrides.plainText,
);
useEffect(
  () =>
    schemeRegistry.onDidChange(() =>
      setLineTextColor(schemeRegistry.getActive().editor.overrides.plainText),
    ),
  [],
);
```

3. :149 与 :178-194 行渲染处 `LINE_TEXT_COLOR` 引用改 `lineTextColor`。
4. 测试同步：`src/__tests__/large-file-viewer.test.tsx` 新增 `方案切换后行文本色响应更新`——渲染组件记录行色 → `schemeRegistry.setActive(...)` 切第二方案（测试注册假 scheme，schemeRegistry.register/_reset 既有模式照抄）→ 断言行色更新为新方案 plainText。
5. 文档同步：`src/panels/editor/CLAUDE.md` CP-022 节「行文本前景色 = active 方案 editor.overrides.plainText，经 schemeRegistry 直取（mdPreviewStyle 先例…）」改写为「渲染期 state + schemeRegistry.onDidChange 订阅响应式取色（FE-03，editorThemeSlot 先例）——硬约束 #6 例外登记维持」。

**验证**：`npx vitest run src/__tests__/large-file-viewer.test.tsx` 绿（新用例）；`rg "LINE_TEXT_COLOR" src/panels/editor/largeFileViewer/` 零命中。

---

### FE-04 · 大文件 sizeBytes 真实字节化（stat 通道，review-04 #4）

`sizeHint = doc.length`（UTF-16 码元数）近似字节数，CJK 文件信息条系统性偏小。根治 = BE-05 的 fs_stat 真实字节通道 + 全链删近似字段（未来最优，不留兼容过渡）。

**位置**：`src/panels/editor/useCodeMirror.ts:55-58`（LargeFileSignal）、`:364`（信号上送）；`src/panels/editor/largeFileViewer/LargeFileViewer.tsx:40-45`（props）、`:152`（信息条）；`src/panels/editor/largeFileViewer/useLineIndex.ts:76/86-89`（死参）；`src/panels/editor/EditorPanel.tsx:47-49`；`src/panels/diff/DiffPanel.tsx:730-734/749-753`；`src/panels/gitshow/GitShowPanel.tsx:300-305`

**现状**：

```ts
export interface LargeFileSignal {
  filePath: string;
  sizeBytes: number;
}
```

```ts
setLargeFile({ filePath, sizeBytes: sizeHint });
```

```ts
// LargeFileViewer.tsx
export interface LargeFileViewerProps {
  filePath: string;
  fileSizeBytes: number;
  sourceLabel: string;
}
// :152 信息条：只读浏览（{formatSize(fileSizeBytes)}）
```

**修复步骤**：

1. useCodeMirror.ts：`LargeFileSignal` 删 `sizeBytes` 字段（仅留 filePath）；`:364` 改 `setLargeFile({ filePath })`。
2. LargeFileViewer.tsx：
   - `LargeFileViewerProps` 删 `fileSizeBytes`；
   - 组件内挂载 stat 取真实大小（与 FE-05 失效比对共用同一份 meta state）：

```tsx
// FE-04/05: 真实文件元数据（fs_stat）——信息条大小展示 + 失效比对基线
const [fileMeta, setFileMeta] = useState<{ sizeBytes: number; mtimeMs: number | null } | null>(null);
useEffect(() => {
  let cancelled = false;
  fs.statFile(filePath)
    .then((m) => { if (!cancelled) setFileMeta({ sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs }); })
    .catch((err) => console.warn("[slTerminal] statFile 失败（信息条大小暂缺）:", filePath, err));
  return () => { cancelled = true; };
}, [filePath]);
```

   - 信息条（:152）改：`只读浏览（{fileMeta !== null ? formatSize(fileMeta.sizeBytes) : "…"}）`；
   - `import { fs } from "../../../ipc"` 接入（blockCache.ts 同款路径形态）。
3. useLineIndex.ts：签名 `useLineIndex(filePath: string, fileSizeBytes: number)` 删第二参（:76），`:86-89` 的 void 注释段删；第二参槽由 FE-05 的 `fileRev` 接替（见 FE-05 步骤 3——同 agent 落地，签名一步到位 `useLineIndex(filePath: string, fileRev: number)`）。
4. 调用点适配：EditorPanel.tsx:47-49 删 `fileSizeBytes={largeFile.sizeBytes}` 行；DiffPanel.tsx:732/751 删 `fileSizeBytes={...}` 行；GitShowPanel.tsx:304 删同款行。
5. GitShowPanel/DiffPanel 的 10MB 判定保留 `text.length`（HEAD blob 无磁盘 stat 通道——登记近似口径）。
6. 测试同步：
   - `large-file-viewer.test.tsx`：全部 `<LargeFileViewer ...>` 渲染点删 fileSizeBytes prop + mock `fs.statFile`（setup 或文件内 vi.mock("../ipc/fs"...) 补 statFile——该测试文件现有 mock 形态照抄）；信息条用例断言改「stat resolve 后显示真实大小」；新增 `statFile 失败时信息条降级为 …` 用例；
   - useCodeMirror 大文件用例（use-code-mirror 测试文件内 largeFile 相关）：`setLargeFile` 断言形态去 sizeBytes；
   - EditorPanel/GitShow/Diff 相关测试若断言 LargeFileViewer props，同步删 fileSizeBytes 断言。
7. 文档同步：`src/panels/editor/CLAUDE.md` CP-022 节「largeFile: { filePath, sizeBytes } | null」改「largeFile: { filePath } | null」；信息条口径句补「大小 = fs_stat 真实字节（FE-04）」；`src/panels/CLAUDE.md` gitshow/diff 节补「blob 侧 10MB 判定用 text.length 近似（HEAD blob 无磁盘 stat 通道）」。

**验证**：

- `rg "sizeBytes" src/panels/editor/useCodeMirror.ts src/panels/editor/EditorPanel.tsx src/panels/diff/DiffPanel.tsx src/panels/gitshow/GitShowPanel.tsx` 仅命中 FE-08 预检处的局部变量（若有）——LargeFileSignal/props 通道零残留；
- `rg "fileSizeBytes" src/panels/editor/largeFileViewer/LargeFileViewer.tsx` 零命中（prop 已删，改 fileMeta）；
- `npx vitest run large-file-viewer use-code-mirror` 绿。

---

### FE-05 · blockCache 文件变更失效机制（review-04 #5）

用户裁决「实现失效机制」。设计：挂载 stat 基线（FE-04 的 fileMeta）→ fs-event Modify 命中 → 重 stat 比对 → 变化即整文件缓存失效 + 行索引复位重扫。事件驱动（useCodeMirror 外部修改链路同通道），静默重载（只读视图无 dirty，重载安全）；事件丢失残余窗口登记为已知边界（editor 域同依赖 fs-event，hooks/CLAUDE.md 已登记 win10 notify 丢事件前科）。

**位置**：`src/panels/editor/largeFileViewer/blockCache.ts`（全文 94 行）、`LargeFileViewer.tsx`、`useLineIndex.ts:91-116`

**现状**：blockCache 全文无任何失效机制（LRU + inflight 单飞，`_resetBlockCache` 仅测试用）；useLineIndex :107-116 以 `fileKey !== filePath` 渲染期复位。

**修复步骤**：

1. blockCache.ts 新增代际失效（追加于 `_resetBlockCache` 前）：

```ts
/** 文件代际（失效计数）——invalidateFile 递增；readBlock 在途任务写缓存前比对，
 *  代际已推进（失效发生于读取途中）则旧代际结果不入缓存（防失效后脏回填） */
const fileGen = new Map<string, number>();

/** 失效指定文件的全部缓存块 + 推进代际（FE-05；inflight 保留——调用方仍收响应，
 *  代际比对阻止其写缓存） */
export function invalidateFile(filePath: string): void {
  fileGen.set(filePath, (fileGen.get(filePath) ?? 0) + 1);
  const prefix = `${filePath}${KEY_SEP}`;
  for (const k of [...cache.keys()]) {
    if (k.startsWith(prefix)) cache.delete(k);
  }
}
```

2. readBlock 的 task 内（:49-61）加代际比对：

```ts
const task = (async () => {
  const gen = fileGen.get(filePath) ?? 0;
  const text = await fs.readFileRange(filePath, blockIndex * READ_BLOCK_BYTES, READ_BLOCK_BYTES);
  // 代际复核（FE-05）：读取途中文件已失效 → 本次结果不回填缓存（防脏写）；
  // 等待期间被并发插入的既有守卫保持
  if ((fileGen.get(filePath) ?? 0) === gen && !cache.has(key)) {
    cache.set(key, text);
    evictIfOverLimit();
  }
  return text;
})();
```

3. `_resetBlockCache` 补 `fileGen.clear();`。
4. useLineIndex.ts：签名改 `useLineIndex(filePath: string, fileRev: number)`；:92/107-116 复合键：

```ts
const [fileKey, setFileKey] = useState(() => `${filePath}#${fileRev}`);
// 文件变更（rev 递增）或换文件 → 全量复位（索引/缓存基线一并重建）
const currentKey = `${filePath}#${fileRev}`;
if (fileKey !== currentKey) {
  setFileKey(currentKey);
  wsRef.current = createWorkspace();
  setSnap({ starts: wsRef.current.starts, eofByte: null, eofEndsWithNewline: false, rev: 0 });
}
```

5. LargeFileViewer.tsx：
   - 加 `const [fileRev, setFileRev] = useState(0);`；`useLineIndex(filePath, fileRev)`；
   - fileMeta（FE-04 步骤 2）即失效比对基线——基线 ref：`const baseMetaRef = useRef<{ sizeBytes: number; mtimeMs: number | null } | null>(null);`，stat effect 的 then 内同步 `baseMetaRef.current = {...}`；
   - fs-event 订阅 effect（照 useCodeMirror.ts:587-623 订阅形态——onFsEvent 回调过滤 Modify 类事件 + 路径归一化比较命中 filePath）：

```tsx
// FE-05: 外部修改 → 缓存失效 + 行索引复位重扫（只读视图无 dirty，静默重载安全；
// 事件丢失残余窗口 = 与 editor 域同款 fs-event 依赖，登记 editor/CLAUDE.md）
useEffect(() => {
  const off = onFsEvent((events) => {
    // 路径归一化比较与 Modify 过滤形态照 useCodeMirror.ts:587-623 先例
    const hit = events.some(/* kind 为 Modify 且路径命中 filePath */);
    if (!hit) return;
    void fs.statFile(filePath).then((m) => {
      const base = baseMetaRef.current;
      if (base !== null && (m.mtimeMs !== base.mtimeMs || m.sizeBytes !== base.sizeBytes)) {
        baseMetaRef.current = { sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs };
        setFileMeta({ sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs });
        invalidateFile(filePath);
        setFileRev((r) => r + 1);
      }
    }).catch(() => { /* stat 失败（文件已删等）——读取路径自行兜底 fatalError */ });
  });
  return off;
}, [filePath]);
```

6. 测试同步（`large-file-viewer.test.tsx` 加 describe「文件变更失效（FE-05）」）：
   - `invalidateFile 清空该文件全部缓存块且不影响他文件`（readBlock 预热两块 → invalidateFile → peekCachedBlock 为 undefined；他文件块保留）；
   - `读取途中失效的旧代际结果不回填缓存`（readFileRange mock 挂起 → invalidateFile → resolve → peek 仍 undefined）；
   - `fs-event Modify 命中后索引复位重建`（mock onFsEvent 捕获回调 → 触发事件 → statFile mock 返回不同 mtime → 断言 readFileRange 重新拉取 + 行数重扫）；
   - `mtime/size 未变的事件不失效`（同 mtime stat → invalidateFile spy/行为断言缓存保留）。
7. 文档同步：`src/panels/editor/CLAUDE.md` CP-022 节补「外部修改失效：fs-event Modify + fs_stat 比对（mtime/size 变化 → 缓存整表失效 + 行索引复位重扫），静默重载（只读语义）；事件丢失残余窗口为已知边界（同 editor 域 fs-event 依赖）」。

**验证**：

- `npx vitest run src/__tests__/large-file-viewer.test.tsx` 绿（含四新增用例）；
- `rg -n "invalidateFile" src/panels/editor/largeFileViewer/` ≥ 2（定义 + LargeFileViewer 消费）；
- `npx tsc --noEmit` 绿。

---

### FE-06 · 注入产物 `</script>` 计数断言（review-04 #6）

拼接纪律 #1（注入段自身不得输出 `</script>`——宿主内容不转义后此为唯一防线）无测试锁：parse-only 的非贪婪正则 `/<script>([\s\S]*?)<\/script>/` 只取首段，提前闭合截断后仍通过 parse。

**位置**：`src/__tests__/doc-viewer-injection.test.ts:23-28`（parseScript）、`:30-74`（段组合 describe）

**现状**：parseScript 只校验首段可 parse；全文件无 `</script>` 计数断言。

**修复步骤**：

1. describe("buildInjectedScript 段组合") 内新增用例：

```ts
it("注入产物不含提前闭合——<script> 与 </script> 各恰好一次（段组合矩阵）", () => {
  // FE-06：拼接纪律 #1 测试锁——非贪婪 parse 只取首段，提前闭合截断静默；
  // 计数断言锁死「注入产物整体恰一对 script 标签」（buildInjectedScript.ts:13-30 纪律 1）
  const combos: Array<[string, Parameters<typeof buildInjectedScript>[1]]> = [
    ["无 extra 段", []],
    ["html fragmentNav", [{ kind: "fragmentNav" }]],
    ["md linkRouter+scrollReport", [{ kind: "linkRouter" }, { kind: "scrollReport" }]],
  ];
  for (const [name, extra] of combos) {
    const out = buildInjectedScript(NONCE, extra);
    expect(out.match(/<script>/g), `${name}：开标签恰一次`).toHaveLength(1);
    expect(out.match(/<\/script>/g), `${name}：闭标签恰一次`).toHaveLength(1);
  }
});
```

2. parseScript 保留不动（SyntaxError 防线与计数断言互补）。
3. 文档同步：`src/panels/docViewer/CLAUDE.md`「注入脚本组装」节纪律 1 句末补「（FE-06 起有计数断言测试锁，doc-viewer-injection.test.ts）」。

**验证**：`npx vitest run src/__tests__/doc-viewer-injection.test.ts` 绿（含新用例）。

---

### FE-07 · useFileTree 续页失败保留首帧（review-02 #4）

loadRoot catch（:170-180）无条件 `setRootNodes([])` + `setDirErrors(rp)`——续页失败时清掉已渲染首帧（且 dirErrors 记根路径 → ExplorerPanel 根错误占位替换整树）。:171 注释自称「续页失败 → 可重试」，语义不成立。

**位置**：`src/features/explorer/useFileTree.ts:141-183`

**现状**：

```ts
try {
  const first = await readDirPage(rp);
  if (gen !== undefined && gen !== genRef.current) return;
  setDirErrors(/* 清 rp 条目 */);
  setRootNodes(toTreeNodes(first.entries));
  let cursor = first.nextCursor;
  while (cursor !== null) { /* 续页拼接 */ }
} catch (err) {
  // FE-07: 根目录加载失败按路径记录错误（首帧失败 → 错误占位；续页失败 → 可重试）
  console.error("[slTerminal] readDirPage 失败:", rp, err);
  const msg = getErrorMessage(err);
  setDirErrors(/* 记 rp 条目 */);
  if (gen === undefined || gen === genRef.current) setRootNodes([]);
}
```

**修复步骤**：

1. loadRoot 内 try 前加局部旗标，catch 分双分支：

```ts
// FE-07: 首帧落地旗标——区分首帧失败（错误占位 + 清空）与续页失败（保留首帧）
let firstFrameCommitted = false;
try {
  const first = await readDirPage(rp);
  if (gen !== undefined && gen !== genRef.current) return;
  setDirErrors(/* 现状不变 */);
  setRootNodes(toTreeNodes(first.entries));
  firstFrameCommitted = true;
  // 续页循环现状不变
} catch (err) {
  console.error("[slTerminal] readDirPage 失败:", rp, err);
  if (firstFrameCommitted) {
    // FE-07: 续页失败——保留已渲染首帧；不记 dirErrors（根错误占位会替换整树）、
    // 不清空。重试经用户刷新（refreshExpanded）或后续展开操作。
    // FE-01 联动：首帧已落即视为加载窗口闭合——抑制由 restoreExpanded 路径正常解除
    return;
  }
  // 首帧失败：现状语义（错误占位 + 清空）+ FE-01 联动解除加载抑制
  if (gen === undefined || gen === genRef.current) {
    restoringRef.current = false;
  }
  const msg = getErrorMessage(err);
  setDirErrors(/* 现状不变 */);
  if (gen === undefined || gen === genRef.current) setRootNodes([]);
}
```

   （注：`return` 在 catch 内直接返回——loadRoot resolve 后 rootPath effect 的 `.then(() => restoreExpanded())` 仍正常执行，FE-01 抑制经 restoreExpanded 解除，逻辑闭环。）
2. 测试同步（use-file-tree.test.ts）：
   - 新增 `续页失败保留首帧`：readDirPage mock 首页 resolve（50 条 + nextCursor）→ 续页 reject → 断言 rootNodes 保留首帧 50 条 + dirErrors 无 rp 条目 + console.error 已打；
   - 新增 `首帧失败维持错误占位语义`：首页 reject → dirErrors 含 rp + rootNodes 空；
   - FE-01 联动用例（首帧失败解除抑制）并入 FE-01 步骤 4 同组落地。
3. 文档同步：`src/features/explorer/CLAUDE.md`「useFileTree 自包含加载」节补一句「loadRoot 分首帧/续页失败双分支：首帧失败 → 错误占位 + 清空；续页失败 → 保留已渲染首帧（不记根错误、不清空），重试经刷新（FE-07）」。

**验证**：`npx vitest run src/__tests__/use-file-tree.test.ts` 绿（含两新增用例）。

---

### FE-08 · useCodeMirror 10MB 检查前置 stat 预检（界外全收项）

现状：先全量 `fs.readFile`（:354）再以 `doc.length` 判定（:358）——>10MB 文件也先进内存再拒绝，「内存保护」不覆盖读盘一步。FE-04 的 stat 通道前置后顺带根治。

**位置**：`src/panels/editor/useCodeMirror.ts:352-377`

**现状**：

```ts
} else if (filePath) {
  try {
    doc = await fs.readFile(filePath);
    if (genRef.current !== gen) return;
    // P2-10: 大文件检查 — UTF-8 文本 length 近似文件字节数
    const sizeHint = doc.length;
    if (sizeHint > MAX_FILE_SIZE_BYTES) {
      filePathRef.current = undefined;
      setLargeFile({ filePath, sizeBytes: sizeHint });
      return;
    } else if (sizeHint > LARGE_FILE_WARN_BYTES) {
      const proceed = await confirmDialog({ /* 约 X MB */ });
      ...
    }
  } catch (err) { ... }
}
```

**修复步骤**：

1. 读盘前插 stat 预检（fs.readFile 之前）：

```ts
} else if (filePath) {
  try {
    // FE-08: stat 预检前置——>10MB 零读盘直接引导只读浏览（原形态先全量读盘再拒绝，
    // 内存保护不覆盖读盘一步）；stat 失败直接落 catch（文件不存在/权限——readFile 必同败）
    const meta = await fs.statFile(filePath);
    if (genRef.current !== gen) return;
    if (meta.sizeBytes > MAX_FILE_SIZE_BYTES) {
      filePathRef.current = undefined;
      setLargeFile({ filePath });
      return;
    } else if (meta.sizeBytes > LARGE_FILE_WARN_BYTES) {
      // 1MB-10MB 警告：真实字节数（原 doc.length UTF-16 近似，CJK 文件系统性偏小）
      const proceed = await confirmDialog({
        title: "打开大文件",
        message: `文件较大（约${(meta.sizeBytes / 1_000_000).toFixed(1)}MB），打开可能影响性能。`,
        confirmText: "继续",
      });
      if (!proceed) {
        doc = `// [slTerminal] 用户取消打开大文件（约${(meta.sizeBytes / 1_000_000).toFixed(1)}MB）。`;
        filePathRef.current = undefined;
      }
    }
    if (filePathRef.current === undefined) return; // 预检拦截/用户取消——不读盘
    doc = await fs.readFile(filePath);
    if (genRef.current !== gen) return;
    // 读后复核（TOCTOU 防线）：stat 与读盘间文件长大超 10MB → 仍引导只读浏览
    if (doc.length > MAX_FILE_SIZE_BYTES) {
      filePathRef.current = undefined;
      setLargeFile({ filePath });
      return;
    }
  } catch (err) { ... 现状不变 ... }
}
```

   注意边界（写死）：`filePathRef.current === undefined` 判定用户取消——现状取消分支已清 filePathRef（:375 先例），照用；useSnapshot 分支（:350-351）不动（快照路径契约「已过检/回填源」panels/CLAUDE.md 已登记）。
2. 测试同步（use-code-mirror 测试文件）：
   - 既有大文件用例适配：mock 补 `fs.statFile`（>10MB 用例断言 **fs.readFile 未被调用** + largeFile 信号 = `{ filePath }`——零读盘语义锁死）；
   - 新增 `stat 预检 1MB-10MB 弹窗用真实字节文案`；
   - 新增 `stat 与读盘间长大（TOCTOU）仍引导只读`：stat mock ≤10MB、readFile 返回 >10MB 字符串 → largeFile 信号置位；
   - 既有 `>1MB 弹窗` 用例的 mock 链补 statFile。
3. 文档同步：`src/panels/editor/CLAUDE.md` 大文件四层防线节「10MB 可编辑上限」条补「预检前置 fs_stat（FE-08）——超限零读盘；读后复核 doc.length 为 TOCTOU 防线」。

**验证**：

- `npx vitest run use-code-mirror` 绿（含三新增/适配用例）；
- `rg -n "await fs.readFile" src/panels/editor/useCodeMirror.ts` 命中行晚于 `await fs.statFile` 命中行（预检前置序锁死）。

---

### FE-09 · WorkspaceDockHost disposables 消费路径接通（review-02 #7）

handleReady 内 disposables 数组（:207）共 6 个 push，其中 :259-265 的自定义清理（apiRef=null/`__dockviewApi`=undefined/unregisterHostApi）**无任何消费路径**——组件卸载后注册表与 window 全局残留旧引用。

**位置**：`src/workspace/WorkspaceDockHost.tsx:199-265`

**现状**：`const disposables: Array<{ dispose(): void }> = [];`（:207）…… `disposables.push({ dispose: () => { apiRef.current = null; window.__dockviewApi = undefined; unregisterHostApi(); } });`（:259-265），数组此后无消费。

**修复步骤**：

1. 组件作用域加 ref（apiRef 声明处附近）：

```tsx
const disposablesRef = useRef<Array<{ dispose(): void }>>([]);
```

2. handleReady 内 :265 之后（自定义清理 push 后）加：`disposablesRef.current = disposables;`
3. 组件级卸载 effect（handleReady 定义之后）：

```tsx
// FE-09: 宿主卸载消费 disposables——事件订阅随 dockview api dispose 自动释放，
// 本通道的生效点 = 自定义清理（apiRef/__dockviewApi/unregisterHostApi 置空）
useEffect(
  () => () => {
    for (const d of disposablesRef.current) d.dispose();
    disposablesRef.current = [];
  },
  [],
);
```

4. 测试同步：`src/__tests__/workspace-host-pages.test.tsx`（真实 dockview 集成先例）新增 `宿主卸载后 __dockviewApi 置空且宿主 API 注销`——render → 断言 `window.__dockviewApi` 非空 → unmount → 断言 `window.__dockviewApi === undefined` + `getPageApi(...)` 返回 null。
5. 文档同步：`src/workspace/CLAUDE.md`「__dockviewApi 宿主唯一不变量」节补一句「宿主卸载经 disposablesRef 消费清理（FE-09）——apiRef/__dockviewApi/unregisterHostApi 置空」。

**验证**：`npx vitest run src/__tests__/workspace-host-pages.test.tsx` 绿（含新用例）；`rg -n "disposablesRef" src/workspace/WorkspaceDockHost.tsx` ≥ 3（声明 + 赋值 + 消费）。

---

### FE-10 · PageDockviewHost 改名 tabChrome（review-02 #9）

S11 共享宿主改造后原「每页一实例宿主组件」消亡，文件仅存页签共享件（DefaultTab/Watermark/RightHeader/页签菜单/applyRename）——名实不符。

**位置**：`src/workspace/PageDockviewHost.tsx`（整体改名）；消费点：`src/workspace/WorkspaceDockHost.tsx:37`、`src/workspace/Workspace.tsx:185`、`src/__tests__/terminal-rename-apply.test.ts:9`、`src/__tests__/workspace-defaulttab.test.tsx:33`、`src/__tests__/workspace-header-actions.test.tsx:43`；注释引用点：`src/lib/panelId.ts:7`、`src/workspace/tabClose.ts:5`、`src/theme/schemes/types.ts:234`、`src/theme/schemes/linear.ts:193`

**修复步骤**：

1. `git mv src/workspace/PageDockviewHost.tsx src/workspace/tabChrome.tsx`；文件头注（:1-8）改写为：「tabChrome — 页签 chrome 共享件（DefaultTab/Watermark/RightHeader/页签菜单/applyRename）。S11 共享宿主改造后原 PageDockviewHost 宿主组件消亡（CP-004），仅存页签共享件，故名随实（FE-10）。」
2. 五处 import 路径替换：`./PageDockviewHost` → `./tabChrome`（WorkspaceDockHost/Workspace）；`../workspace/PageDockviewHost` → `../workspace/tabChrome`（三测试文件）。
3. 注释引用替换：`lib/panelId.ts:7`「见 workspace/PageDockviewHost.makeTerminalIdInPage」→「见 workspace/pageGroups.ts（makeTerminalIdInPage 真实所在）」（该引用本就漂移——函数实际在 pageGroups.ts，顺带修正）；`tabClose.ts:5`「原内联于 PageDockviewHost DefaultTab ×」→「原内联于 tabChrome DefaultTab ×」；`theme/schemes/types.ts:234` 与 `linear.ts:193` 的「PageDockviewHost 挂载点/根 div」→「tabChrome 挂载点/根 div」。
4. **knip.json 同步（计划期补查发现的消费方，勿漏）**：`knip.json:211` 的 ignore 键 `"src/workspace/PageDockviewHost.tsx"` → `"src/workspace/tabChrome.tsx"`（值不变）——不改则该文件未用导出的 ignore 失效，knip 报新红。
5. 测试文件头注同步：terminal-rename-apply.test.ts:3、workspace-defaulttab.test.tsx:3 的「PageDockviewHost.tsx 导出」表述改 tabChrome.tsx。
6. 文档同步：`src/workspace/CLAUDE.md` 无 PageDockviewHost 文本引用（grep 零命中实证）——不动；compromises.md CP-036 注记的「PageDockviewHost 两 action」为历史记录，不动。
7. 测试同步：三测试文件仅 import 路径与头注改名，用例零改动。

**验证**：

- `rg "PageDockviewHost" src/ knip.json` 零命中（含注释与 knip ignore 键）；
- `npx tsc --noEmit` + `npx vitest run terminal-rename-apply workspace-defaulttab workspace-header-actions` 绿；
- `npx knip --production` 无新增红（tabChrome.tsx 未用导出仍被 ignore 压住）。

---

### FE-11 · exportContextBindings 退役面清干净（review-03 #3）

`exportContextBindings` 生产零消费（「供未来 iframe 转发脚本用」——CP-013 后键盘不跨窗口，该未来永不至），残留：方法本体 + `ExportedBinding` 类型 + 接口成员 + barrel re-export + CLAUDE.md 过时描述 + 三测试面。

**位置**：`src/features/shortcuts/ShortcutRegistry.ts:155-164`；`src/features/shortcuts/types.ts:67-71`（ExportedBinding）、`:93-94`（接口成员）；`src/features/shortcuts/index.ts:11`；`src/features/shortcuts/CLAUDE.md`（「前向接口」节 + 测试模式节列举词）；`src/__tests__/shortcuts.test.ts:669-730`；`src/__tests__/html-panel.test.tsx:88-92`；`src/__tests__/markdown-panel.test.tsx:102` 区

**现状**：

```ts
/** 导出某 context 当前生效的绑定（含 global，排除解绑），供未来 iframe 转发脚本用 */
exportContextBindings(context: ShortcutContext): ExportedBinding[] { ... }
```

**修复步骤**：

1. 删 ShortcutRegistry.ts:155-164 方法本体；删 types.ts:67-71 + :93-94；删 index.ts:11 的 `ExportedBinding` re-export。
2. shortcuts/CLAUDE.md：「前向接口」节删 exportContextBindings 整行（「供 HtmlPanel postMessage 键盘转发」描述已过时——键盘不跨窗口 CP-013）；「测试模式」节「核心」行的列举词 `、exportContextBindings` 删除。
3. shortcuts.test.ts :669-730 甄别处理（写死）：describe "exportContextBindings / listCommands" 块内 **exportContextBindings 用例删除、listCommands 用例保留**——describe 名改为 "listCommands"，listCommands 断言原样。
4. html-panel.test.tsx:88-92 与 markdown-panel.test.tsx:102 区的 mock：先 grep 两面板生产代码（PreviewFrame/HtmlPanel/MarkdownPanel）是否仍 import shortcuts——CP-013 后应为零消费：
   - 零消费 → 整个 `vi.mock("../features/shortcuts/ShortcutRegistry", ...)` 块删除（含 vi.hoisted 内 mockExportContextBindings 定义点）；
   - 仍有消费 → mock 对象仅删 `exportContextBindings` 字段。
5. 验证时若 knip 报告 ExportedBinding 相关新红，随改。

**验证**：

- `rg "exportContextBindings|ExportedBinding" src/` 零命中；
- `npx vitest run shortcuts html-panel markdown-panel` 绿；
- `npx knip --production` 无新增红。

---

## TE 域（测试与工具链，9 项）

### TE-01 · check-ts7-trigger 补 HTTP 状态码校验（review-01 #1）

`getJson`（scripts/check-ts7-trigger.mjs:24-40）不校验 statusCode——GitHub 限流 403 返回合法 JSON，误落「未达成」（退出码 1）而非登记的「查询失败」（退出码 2）。

**位置**：`scripts/check-ts7-trigger.mjs:23-40`；`scripts/check-ts7-trigger.d.mts`；`src/__tests__/deps-ts7-trigger.test.ts`

**现状**：

```js
function getJson(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "user-agent": "slterminal-trigger-check" } }, (res) => {
        let body = "";
        res.on("data", (c) => (body += c));
        res.on("end", () => {
          try { resolve(JSON.parse(body)); } catch (e) { reject(e); }
        });
      })
      .on("error", reject);
  });
}
```

**修复步骤**：

1. 模块顶部 import 加 `import http from "node:http";`；getJson 改协议分派 + 状态码守卫，并导出（测试可达）：

```js
/** 极简 GET JSON(无外部依赖)——非 2xx 一律视为查询失败(落退出码 2 未知态):
 *  限流 403 等错误页可能返回合法 JSON,误解析会假报「未达成」(退出码 1)(TE-01) */
export function getJson(url) {
  return new Promise((resolve, reject) => {
    const mod = url.startsWith("https:") ? https : http; // http 分支仅供测试本地桩
    mod
      .get(url, { headers: { "user-agent": "slterminal-trigger-check" } }, (res) => {
        const status = res.statusCode ?? 0;
        if (status < 200 || status >= 300) {
          res.resume(); // 排空响应防连接占用
          reject(new Error(`HTTP ${status}`));
          return;
        }
        let body = "";
        res.on("data", (c) => (body += c));
        res.on("end", () => {
          try { resolve(JSON.parse(body)); } catch (e) { reject(e); }
        });
      })
      .on("error", reject);
  });
}
```

2. `check-ts7-trigger.d.mts` 补声明：`export function getJson(url: string): Promise<unknown>;`（main 消费形态不变——`issue?.state` 等可选链不变）。
3. 测试同步：`src/__tests__/deps-ts7-trigger.test.ts` 新增 describe("getJson HTTP 状态码守卫")：

```ts
it("getJson_非2xx_reject含HTTP状态码", async () => {
  const server = http.createServer((_req, res) => {
    res.writeHead(403, { "content-type": "application/json" });
    res.end('{"message":"API rate limit exceeded"}');
  });
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  try {
    const port = (server.address() as import("node:net").AddressInfo).port;
    await expect(getJson(`http://127.0.0.1:${port}/`)).rejects.toThrow("HTTP 403");
  } finally {
    server.close();
  }
});

it("getJson_200_正常解析", async () => { /* 同款桩返 200 JSON → resolve 对象 */ });
```

   （import http from "node:http" + getJson 自 mjs——vitest jsdom 环境 node 内置模块可用。）
4. 文档同步：脚本头注（:3-6）退出码 2 语义句补「（含非 2xx 响应——TE-01）」。

**验证**：`npx vitest run src/__tests__/deps-ts7-trigger.test.ts` 绿（7 用例）；`node scripts/check-ts7-trigger.mjs` 实跑一次退出码 ∈ {0,1,2}（网络可达时 1）。

---

### TE-02 · d.mts 入参声明补 null（review-01 #2）

**位置**：`scripts/check-ts7-trigger.d.mts:11-14`；`src/__tests__/deps-ts7-trigger.test.ts:35`

**现状**：

```ts
export function evaluateTrigger(
  issueState: string | undefined,
  latestVersion: string | undefined,
): EvaluateTriggerResult;
```

运行期 main 传 `registry?.["dist-tags"]?.latest`（可 null），测试 :35 以 `null as unknown as string` 绕声明。

**修复步骤**：

1. 声明改：

```ts
export function evaluateTrigger(
  issueState: string | null | undefined,
  latestVersion: string | null | undefined,
): EvaluateTriggerResult;
```

2. 测试 :35 删 cast：`evaluateTrigger(undefined, null)`。
3. 文档同步：无。

**验证**：`npx tsc --noEmit` 绿；`rg "as unknown as string" src/__tests__/deps-ts7-trigger.test.ts` 零命中。

---

### TE-03 · ci.yml 补 src/types 漂移守卫 step（review-05 #1）

CP-024 销项称「导出测试 + git diff 漂移守卫入门禁」，实际 ci.yml 无对应 step（对照同批 CP-033 KaTeX 守卫在 ci.yml:59-63 有实体）。

**位置**：`.github/workflows/ci.yml:63`（KaTeX 守卫 step 之后插入）

**现状**（KaTeX 先例，:59-63）：

```yaml
      # KaTeX 内联产物 diff 守卫（CP-033）：katex 升级漏跑 gen 脚本即红
      - name: Guard — KaTeX 内联产物与生成脚本一致
        run: |
          node scripts/gen-katex-inline.mjs
          git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts
```

**修复步骤**：

1. KaTeX 守卫 step 后插入：

```yaml
      # ts-rs DTO 漂移守卫（CP-024）：改 Rust DTO 漏跑导出即红
      # （定向形态 = --test lib_tests 显式 target，TQ-COV-06 红线不踩）
      - name: Guard — src/types 生成物与 Rust DTO 一致
        run: |
          cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests export_bindings -- --test-threads=1
          git diff --exit-code -- src/types
```

2. 测试同步：CI 配置无本地自动化测试；本地手动验证 = 顺序执行两条命令（导出幂等 → diff 绿）。
3. 文档同步：`src/types/CLAUDE.md:16` 与 `src-tauri/src/CLAUDE.md:71` 的守卫操作指令句补「（CI 门禁 step 已落地——ci.yml Guard — src/types）」；compromises.md CP-024 注记不动（「入门禁」自此成真）。

**验证**：本地 `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests export_bindings -- --test-threads=1` + `git diff --exit-code -- src/types` 全绿；`rg -n "src/types 生成物" .github/workflows/ci.yml` 命中；CI 实跑绿（人工验证点）。

---

### TE-04 · 真实屋哨兵键集合补 env（review-05 #2）

套件自身经 writeFakePlanEnv 向 user 层 settings.json 写 `env` 键，哨兵却仅 `["hooks", "statusLine"]`——写入链失控落真屋时漏检。裁决：套件写入面全量入哨兵；误报面回归（外部并发改 env 报红）登记为已知限制。

**位置**：`e2e-tests/run-wdio.cjs:83-86`（哨兵声明 + 注释）、`:143-144`（verifyRealHomeUnchanged 注释）；`e2e-tests/CLAUDE.md`「防复发校验（CP-046 键级断言）」节 + 「已知并发误报面」节

**现状**：

```js
/** 本套件泄漏判定哨兵键——E2E 唯一可能写入真实屋 settings.json 的键
 *  (hooks 注入 matcher / statusLine 桥接;其余键(env/permissions/用户配置)
 *  外部并发修改合法,不做整文件 diff——CP-046 键级断言口径) */
const SETTINGS_SENTINEL_KEYS = ["hooks", "statusLine"];
```

**修复步骤**：

1. `:86` 改 `["hooks", "statusLine", "env"]`；:83-85 注释改写为：「本套件泄漏判定哨兵键——E2E 可能写入真实屋 settings.json 的全部键（hooks 注入 matcher / statusLine 桥接 / writeFakePlanEnv 假 env；TE-04 补 env 前该写入面漏检）。其余键（permissions/用户配置）外部并发修改合法；env 入哨兵后外部并发改 env 会报红——已知限制：e2e 运行期间勿动 claude env 配置」。
2. :143-144 注释「只比对哨兵键」表述不动（语义仍真），键列举处同步（若有）。
3. 文档同步（e2e-tests/CLAUDE.md）：「防复发校验」节「哨兵键（hooks/statusLine，本套件唯一可能写入的键）」→「哨兵键（hooks/statusLine/env，本套件全部写入面——TE-04）」；「已知并发误报面」节改写——env 从合法放行面移除：「开发者跑 e2e 的同时自己开真实 claude/slterminal 改写非哨兵键（permissions/用户配置）仍合法放行；改写 env 键（如手动改 ANTHROPIC_* 配置）会命中哨兵报红——已知限制（TE-04），e2e 运行期间勿动」。
4. 测试同步：哨兵逻辑无 L2 面；验证 = 人工负向验证（改真实屋 env 哨兵键 → exit 报红）——登记 Stage 06 人工验证点（与 review 待人工清单 S03/CP-046 项联动）。
5. 文档同步另处：`.claude/test-exemptions.md:70` 键级口径修订归 DOC-10（Stage 07 同口径一次改）。

**验证**：`rg -n '"hooks", "statusLine", "env"' e2e-tests/run-wdio.cjs` 命中；`rg -n "唯一可能写入" e2e-tests/` 零命中（失实声明绝迹）。

---

### TE-05 · TQ-E-10 探针首 worker fast-fail + 其余降级（review-05 #4）

多 `--spec` 定向形态每 spec 独立 worker/应用实例，第 2 位实例在 Windows 前台锁定期内必然失焦 → 探针确定性 fast-fail（官方形态收窄）。用户裁决「首 worker 探针 + 其余降级」。一手证据：`node_modules/@wdio/local-runner/build/index.js:257` 实证 `WDIO_WORKER_ID: cid`（cid 形如 `"0-0"`，maxInstances=1 下首 worker = `"0-0"`）。

**位置**：`e2e-tests/wdio.conf.ts:82-90`

**现状**：

```ts
const focused = await browser.execute(() => document.hasFocus());
if (focused !== true) {
  throw new Error(
    "[wdio] 应用窗口未前台聚焦——E2E 运行前提不满足(TQ-E-10 探针)。...",
  );
}
```

**修复步骤**：

1. :85-90 改写：

```ts
const focused = await browser.execute(() => document.hasFocus());
if (focused !== true) {
  // TE-05: 首 worker 维持 fast-fail；多 --spec 形态第 2 位起的 worker（独立应用
  // 实例，Windows 前台锁定期内必失焦）降级 warn 继续——其 $ 族命令逐命令吃 +5s
  // focus 惩罚（e2e-tests/CLAUDE.md 登记），多 spec 形态仍不推荐但不再确定性失败。
  // WDIO_WORKER_ID = local-runner spawn 的 cid（"0-0" 为首 worker，实证
  // @wdio/local-runner build/index.js:257）；env 缺失按首 worker 处理（单 worker
  // 场景维持探针原语义）
  const workerId = process.env.WDIO_WORKER_ID;
  const isFirstWorker = workerId === undefined || workerId.startsWith("0-");
  if (isFirstWorker) {
    throw new Error(
      "[wdio] 应用窗口未前台聚焦——E2E 运行前提不满足(TQ-E-10 探针)。请先聚焦 slTerminal 窗口再跑 npm run e2e;CI 环境请确认 embedded driver 启动后窗口置前。",
    );
  }
  console.warn(
    `[wdio] TQ-E-10 探针降级：worker ${workerId} 前台聚焦不可用（多 spec 形态非首实例必失焦）——本 worker $ 族命令将逐命令吃 +5s focus 惩罚`,
  );
}
```

2. 测试同步：wdio.conf.ts 不在 L2 覆盖内（e2e-tests/ 不在根 tsconfig/vitest include）；验证 = L4 实跑（见下）。
3. 文档同步（e2e-tests/CLAUDE.md）：「定向运行官方形态 = 单 --spec」节改写——多 --spec 形态「第 2 位 worker 探针降级 warn 继续（TE-05），但其 $ 族命令全吃 +5s/命令 focus 惩罚（长链 spec 会拖出 timeout 预算）——仍推荐单 spec 分次/全量 config 数组形态」；「$()/elementClick 触发焦点检查」节末句「前提满足后 `$` 族命令正常速度」按 TE-09 步骤一并改写（同节两处改动同 agent 落地）。
4. compromises.md CP-030 注记登记归 DOC-08（Stage 07）。

**验证**：

- `node e2e-tests/run-wdio.cjs --spec settings.e2e.ts --spec hooks.e2e.ts` 双 spec 形态：第 2 位 worker 不再 fast-fail（日志含降级 WARN），两 spec 按各自内容通过/失败（不再有「谁在第 2 位谁失败」确定性）；
- 单 spec/全量形态零回归：`npm run e2e` 全绿。

> **落地复核（2026-09-09，S06）**：cid 语义复核证伪本条「maxInstances=1 下首 worker = `"0-0"`、其余非 `"0-"` 前缀」的隐含假设——cid 实为 `${capabilityIndex}-${workerOrdinal}`（`@wdio/cli build/index.js:963-976`/`:1177-1182`），本配置单 capability → **全部 worker 的 cid 均 `"0-N"`**，`startsWith("0-")` 判定致降级分支不可达、全量 15 worker 全 fast-fail（throw 位于 reset 之前 → `resetProjects/resetSettings` 被跳过 → 跨 spec 状态累积，tab-menu 失败）。落地形态改为 `workerId === undefined || workerId === "0-0"`（实文见 `e2e-tests/wdio.conf.ts`）；本条步骤 1 代码块字面不回溯改写（历史原文保留）。

---

### TE-06 · writeFakePlanEnv 收编单点（界外全收项）

settings.e2e.ts:202-217 与 background-tasks.e2e.ts:263-275 两副本逐字重复（后者缺 mkdirSync——solo 跑必 ENOENT，settings 版注释实证）。收编点**不能**是 e2e-tests/helpers.ts——该文件经 VITE_E2E 打包进前端（浏览器侧，import React + window 全局），Node fs 函数入包即破构建；收编点 = 新建 `e2e-tests/node-helpers.ts`（Node runner 侧公共件）。

**位置**：新建 `e2e-tests/node-helpers.ts`；`e2e-tests/settings.e2e.ts:200-217`；`e2e-tests/background-tasks.e2e.ts:261-275`

**修复步骤**：

1. 新建 `e2e-tests/node-helpers.ts`：

```ts
// node-helpers.ts — E2E Node runner 侧公共件（与浏览器侧 helpers.ts 分工：
// helpers.ts 经 VITE_E2E 打包进前端挂 window 全局（禁 Node API）；
// 本文件仅在 wdio runner Node 进程内被 spec import，禁止经任何前端路径打包）

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

/** 写假值余量 env 到 user 层 settings.json（SEC-18 假值占位符；deepseek URL 命中
    QUERIES 匹配集 → 后端刷新后产出占位行 → 导航树余量 footer 可断言）。
    TE-06 收编单点（原 settings/background-tasks 两副本，后者缺 mkdirSync solo 必
    ENOENT——以 settings 版为基） */
export function writeFakePlanEnv(claudeSettingsPath: string): void {
  let root: Record<string, unknown>;
  try {
    root = JSON.parse(readFileSync(claudeSettingsPath, "utf8")) as Record<string, unknown>;
  } catch {
    root = {};
  }
  const env = (root.env ?? {}) as Record<string, unknown>;
  env.ANTHROPIC_BASE_URL = "https://api.deepseek.com/anthropic";
  env.ANTHROPIC_AUTH_TOKEN = "sk-test-e2e"; // 假值占位符（SEC-18，非真实凭据）
  root.env = env;
  // 假屋为 per-pid 唯一新目录——solo 跑时 .claude 未必存在，先建父目录（2026-09-08 实证：
  // 全量队列中 hooks spec 先行建目录故既往全量绿、单 spec 独立假屋必 ENOENT）
  mkdirSync(dirname(claudeSettingsPath), { recursive: true });
  writeFileSync(claudeSettingsPath, JSON.stringify(root, null, 2), "utf8");
}
```

2. settings.e2e.ts：删 :200-217 本地函数，顶部 import 加 `import { writeFakePlanEnv } from "./node-helpers";`，调用点改 `writeFakePlanEnv(claudeSettingsPath)`。
3. background-tasks.e2e.ts：同款（删 :261-275 本地副本 + import + 调用点参数化）。
4. 文档同步：`e2e-tests/CLAUDE.md`「E2E helper 命名与挂载位置」节补一条：「Node runner 侧公共件（fs/os 操作）收 `node-helpers.ts`——与浏览器侧 helpers.ts（VITE_E2E 打包进前端）分工，Node API 禁止进 helpers.ts」。
5. 测试同步：行为不变量由既有用例承载（settings ④/background-tasks C 用例全绿即回归）；`node e2e-tests/run-wdio.cjs --spec background-tasks.e2e.ts` solo 实跑验证 mkdirSync 对齐（原副本缺 mkdirSync 的 solo ENOENT 破口闭合）。

**验证**：

- `rg -n "function writeFakePlanEnv" e2e-tests/` 仅命中 node-helpers.ts；
- `node e2e-tests/run-wdio.cjs --spec settings.e2e.ts` 与 `--spec background-tasks.e2e.ts` 各自 solo 绿。

---

### TE-07 · vitest.config.ts 坏 exclude 清理（界外全收项）

**位置**：`vitest.config.ts:7`

**现状**：`exclude: ['node_modules', 'datalearncodeterax-ai-temp', '.temp', 'e2e-tests'],`——`'datalearncodeterax-ai-temp'` 为去斜杠拼接损坏产物，从未生效。

**修复步骤**：删除该元素 → `exclude: ['node_modules', '.temp', 'e2e-tests'],`。

**测试同步**：`npm test` 全绿（用例集不变）。

**文档同步**：无。

**验证**：`rg "datalearncodeterax" vitest.config.ts` 零命中；`npm test` 用例数与基线一致（199 文件/3236 例量级，无漂移）。

---

### TE-08 · KaTeX 字体真实加载锚点补强（review-01 #6，两分支实证驱动）

markdown.e2e.ts:114-115 仅断言「内容串含 data:font/woff2;base64,」（串存在 ≠ 字体真实加载渲染）。本项为**实证驱动条目**：第一步实跑分支 a 探针定通道，两分支代码均写死如下。

**位置**：`e2e-tests/markdown.e2e.ts:109-115`；分支 b 波及：`src/panels/docViewer/buildInjectedScript.ts`、`src/panels/docViewer/previewMessages.ts`、`src/panels/docViewer/PreviewFrame.tsx`、`src/__tests__/doc-viewer-preview-messages.test.ts`、`src/__tests__/markdown-panel.test.tsx`

**修复步骤**：

1. **实证先行（分支 a 探针）**：临时在 markdown.e2e.ts :115 后插入探针（验证后可去留——分支 a 成立则固化为常驻断言）：

```ts
// TE-08 实证探针：宿主页 document.fonts 是否覆盖 iframe srcdoc 文档的字体加载
await browser.switchToWindow(previewWindowLabel(panelId));
const probe = await browser.execute(() => ({
  katex: document.fonts.check('12px "KaTeX_Main"'),
  control: document.fonts.check('12px "NoSuchFontXyzQq"'),
}));
await browser.switchToWindow("main");
console.log("[TE-08 探针]", JSON.stringify(probe));
```

   判定：`control === false` 证明 check 语义有效；`katex === true` → **分支 a 成立**（宿主页 FontFaceSet 覆盖 iframe——实证定案）；`katex === false` → **转分支 b**。
2. **分支 a（宿主页 fonts.check 成立）**：探针固化为常驻断言（:115 后）：

```ts
// KaTeX 字体真实加载锚点（TE-08/CP-033）：预览域 document.fonts.check 双对照——
// 对照组（不存在字体）必 false 证明 check 语义有效；KaTeX_Main true = 字体真实可渲染
await browser.switchToWindow(previewWindowLabel(panelId));
try {
  const fontLoaded = await browser.execute(() => document.fonts.check('12px "KaTeX_Main"'));
  const controlMiss = await browser.execute(() => document.fonts.check('12px "NoSuchFontXyzQq"'));
  expect(controlMiss).toBe(false);
  expect(fontLoaded).toBe(true);
} finally {
  await browser.switchToWindow("main");
}
```

   证据要求：分支 a 的实证输出（探针 JSON）记入 commit body。
3. **分支 b（宿主页 fonts 不覆盖 iframe）**：VITE_E2E 门控探针段 + 上行白名单第四类型：
   - `buildInjectedScript.ts` 新增可选段 kind `"fontProbe"`（拼接纪律同既有段：完整语句 + 分号收尾、无 `</script>`、nonce 经 JSON.stringify）：

```ts
// fontProbe 段（仅 E2E 拼装传入——生产零注入面）：iframe 内 fonts.check 上行
`document.fonts.ready.then(function(){var ok=document.fonts.check('12px "KaTeX_Main"');parent.postMessage({type:${JSON.stringify(FONT_PROBE_MSG_TYPE)},nonce:${nonceJson},loaded:ok},"*");});`
```

   - `previewMessages.ts`：`FONT_PROBE_MSG_TYPE = "slterm_font_probe"` 常量 + UPLINK 白名单加该类型；
   - `MarkdownPanel` 拼装点：`import.meta.env.VITE_E2E` 时 extra 段追加 `{ kind: "fontProbe" }`（生产构建 tree-shake 零成本）；
   - `PreviewFrame.tsx` 上行处理加分支：type === FONT_PROBE_MSG_TYPE → nonce 校验后经 E2E_ENABLED 门控写 `(window as unknown as { __slterm_e2e_fontProbe?: Record<string, boolean> }).__slterm_e2e_fontProbe = { ...(既有), [label]: loaded }`；
   - markdown.e2e.ts 断言（主窗上下文）：`await browser.waitUntil(async () => browser.execute((l) => (window as ...).__slterm_e2e_fontProbe?.[l] === true, previewWindowLabel(panelId)), { timeout: 10000 })`；
   - 测试同步：`doc-viewer-preview-messages.test.ts` 白名单守卫集合改四项（含 slterm_font_probe）；`markdown-panel.test.tsx` 补「VITE_E2E 拼装含 fontProbe 段」用例；`doc-viewer-injection.test.ts` FE-06 矩阵补 fontProbe 组合（若分支 b 落地）；`html-panel.test.tsx` 上行分派补 fontProbe 分支用例。
4. 文档同步：`e2e-tests/CLAUDE.md`「多 webview WDIO 可达性」节补 TE-08 实证结论一句（哪分支成立 + 锚点形态）；`src/panels/markdown/CLAUDE.md` 或 docViewer/CLAUDE.md 相应登记（分支 b 落地时：fontProbe 段 E2E 门控语义 + 白名单第四类型登记）。

**验证**：

- 分支 a：`node e2e-tests/run-wdio.cjs --spec markdown.e2e.ts` 绿（双对照断言过）；commit body 含探针实证输出；
- 分支 b：同上 + L2 `npx vitest run doc-viewer-preview-messages markdown-panel html-panel doc-viewer-injection` 绿。

---

### TE-09 · CP-030 矛盾句改写 + WARN 计数可观测化（review-05 #3）

e2e-tests/CLAUDE.md:116 同条自相矛盾（「前提满足后 $ 族命令正常速度」已被同条修正段否证）；性能回归感知 sole 依赖 `this.timeout(120000)` 宽松预算。WARN 来源实证：`node_modules/@wdio/tauri-service/dist/cjs/index.js:3133`（包内 logger，不可 patch）——计数接入点只能是 run-wdio.cjs 的 stdio（现 execSync/spawn 均 `stdio: 'inherit'` 直通，无法计数）。

**位置**：`e2e-tests/run-wdio.cjs:296-333`（runWdio + fallback 双通道）；`e2e-tests/CLAUDE.md`「$()/elementClick 触发焦点检查」节（:116 长行）

**修复步骤**：

1. run-wdio.cjs 加计数器（顶部工具函数区）：

```js
// TE-09: tauri-service "core.invoke not available after 5s" WARN 计数（性能回归感知
// 可观测化——focus 命令面扩大 = 计数显著超基线）。stdio 改 pipe 转发以计数；
// FORCE_COLOR=1 保子进程日志着色（pipe 后非 TTY 失色）
let coreInvokeWarnCount = 0;
function wireWarnCounting(src, dst) {
  src.on('data', (chunk) => {
    const s = chunk.toString();
    coreInvokeWarnCount += (s.match(/core\.invoke not available after 5s/g) ?? []).length;
    dst.write(s);
  });
}
```

2. `runWdio(nodeBin)`（:296-304）execSync 改 spawn 转发：

```js
function runWdio(nodeBin) {
  const wdioCli = path.resolve(__dirname, '..', 'node_modules', '@wdio', 'cli', 'bin', 'wdio.js');
  const child = spawn(nodeBin, [wdioCli, 'run', wdioConfig, ...process.argv.slice(2)], {
    stdio: ['inherit', 'pipe', 'pipe'],
    env: { ...process.env, FORCE_COLOR: '1' },
  });
  wireWarnCounting(child.stdout, process.stdout);
  wireWarnCounting(child.stderr, process.stderr);
  child.on('close', (code) => {
    console.log(`[wdio-launcher] tauri-service core.invoke WARN 计数 = ${coreInvokeWarnCount}（基线登记见 e2e-tests/CLAUDE.md——显著超基线 = focus 命令面扩大，排查 $/click 新增点）`);
    process.exit(code ?? 1);
  });
}
```

   （spawn 已在文件顶部 import——:328 现状用了 spawn；execSync 引用删除。注意 `cliArgs` 拼串变量在 spawn 数组形态下弃用，改 `...process.argv.slice(2)`。）
3. `fallback()`（:326-333）同款接线：`stdio: 'inherit'` 改 `['inherit', 'pipe', 'pipe']` + 两路 wireWarnCounting + close 回调补同款计数行（注意 fallback 的 close 回调已有 `process.exit(code)`——计数行插在 exit 前）。
4. e2e-tests/CLAUDE.md「$()/elementClick 触发焦点检查」节改写：
   - 「运行前提 = 窗口前台聚焦——wdio.conf beforeSuite TQ-E-10 探针 fast-fail 保证（失焦即报错退出，不静默吃延迟）；前提满足后 `$` 族命令正常速度，cli-aliases 已回归真实手势（CP-030）」→「运行前提 = 窗口前台聚焦——wdio.conf beforeSuite TQ-E-10 探针 fast-fail 保证（首 worker 失焦即报错退出，多 spec 形态非首 worker 降级 warn 继续——TE-05）；**即便探针通过，每 focus 命令仍确定性 +5s**（`__TAURI_INTERNALS__.core` 缺失致窗口态查询恒失败，与聚焦无关——2026-09-08 修正段口径为准），cli-aliases 已回归真实手势（CP-030，预算经 suite 级 this.timeout(120000) 承接）」；
   - 节末补：「WARN 可观测化（TE-09）：run-wdio 结束打印 core.invoke WARN 计数行；基线 = cli-aliases 单 spec ≈ 13 次（2026-09-08 实测），全量套件基线执行期实跑填记——计数显著超基线 = focus 命令面扩大」。
5. 测试同步：launcher 层无 L2 面；验证 = L4 实跑输出含计数行。

**验证**：

- `node e2e-tests/run-wdio.cjs --spec cli-aliases.e2e.ts` 输出末尾含 `[wdio-launcher] tauri-service core.invoke WARN 计数 = N`（N ≥ 0）且退出码随 wdio 结果；
- `rg -n "正常速度" e2e-tests/CLAUDE.md` 零命中（矛盾句绝迹）；
- `npm run e2e` 全量绿（stdio 转发改造无回归）。

---

## DOC 域（文档登记，11 项——Stage 07 统一收尾）

### DOC-01 · compromises.md CP-002 括注失实修正（review-01 #3）

**位置**：`docs/compromises.md:21`

**现状**：括注「（jsonSchemaCm.ts 五导出，JsonMode 生产接线）」——实际七导出、仅二处生产接线。

**修复步骤**：括注改「（jsonSchemaCm.ts 七导出——二处生产接线，余五导出仅测试直驱）」。

**验证**：`rg -n "jsonSchemaCm.ts 五导出" docs/compromises.md` 零命中；新表述命中。

---

### DOC-02 · .temp/node22.bak 删除 + CP-003 注记收口（review-01 #4）

**位置**：`.temp/node22.bak`（untracked 残留目录）；`docs/compromises.md:23`

**修复步骤**：

1. 若 `.temp/node22.bak` 存在则删除（untracked——纯文件系统动作，无 git 操作）；`.temp/node22` 预置通道**保留**（run-wdio.cjs:306-323 预置逻辑仍在服役，头注 :3 描述有效功能不动）。
2. compromises.md:23 CP-003 注记末句「原便携 Node 22 隔离于 .temp/node22.bak 待全量 e2e 确认后删」→「原便携 Node 22 备份 .temp/node22.bak 已于全量 e2e 确认后删除（2026-09-09）」。

**验证**：`.temp/node22.bak` 不存在；`rg -n "待全量 e2e 确认后删" docs/compromises.md` 零命中。

---

### DOC-03 · CP-033 红测演练补做 + commit body 留痕（review-01 #5）

verify/stage-10.md:15 声称红测演练「在 commit body 留痕」，全链 commit 实查不存在。

**位置**：执行动作（无代码改动）；`docs/compromises-fix/workflows/verify/stage-10.md:15`（口径回写与 DOC-04 一并）

**修复步骤**：

1. 补做红测演练并留痕（Stage 07 执行动作，命令序列）：
   - 手改 `src/panels/markdown/generated/katexInlineCss.ts` 任意一字符（如注释内加字符）；
   - `git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts` → **应非 0**（守卫抓漂移，红测阳性）；
   - `git checkout -- src/panels/markdown/generated/katexInlineCss.ts` 还原；
   - `node scripts/gen-katex-inline.mjs && git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts` → **应 0**（生成幂等归位）。
2. 演练四行输出（命令 + exit code）写入 Stage 07 commit body。

**验证**：演练四步 exit code 与预期一致（第 2 步非 0、第 4 步 0）且记录完整（verify/stage-07.md 断言）；演练输出写入 Stage 07 commit body 属主 agent 自查项（commit 在 Stage verify 后发生——verify/stage-07.md「主 agent 自查项」节登记）。

---

### DOC-04 · CP-033 B2 口径回写（review-01 #7）

checklist/verify 的 B2 字面断言（「新 webview 局部 CSP 放行 font-src data:」）与执行期实际落地（预览域零 CSP）不符，执行文档未回写。SEC-02 落地后形态已成立（宿主页 CSP meta 含 font-src data:）——回写真实落地口径。

**位置**：`docs/compromises-fix/checklist.md:1915`；`docs/compromises-fix/workflows/verify/stage-10.md:15`

**修复步骤**：

1. verify/stage-10.md:15 的 CP-033 行 B2 分支句后补：「（落地复核 2026-09-09：tauri 2.11 无 per-webview CSP 配置面——B2 实际形态 = 宿主页 CSP meta（preview.rs HOST_PAGE）：default-src 'none'; script-src/style-src 'unsafe-inline'; img-src data:; font-src data:，SEC-02 落地；红测演练留痕于 docs/compromises-fix-review-fix Stage 07 commit body）」。
2. checklist.md:1915（B2 对应行）同款回写一句。
3. 注：compromises-fix/ 为历史执行档——回写以「落地复核」注记形式追加，不改写历史断言原文（防篡改历史记录语义）。

**验证**：两文件 grep「落地复核 2026-09-09」各命中一处。

---

### DOC-05 · compromises.md CP-004 注记补页内分屏消亡登记（review-02 #2）

页内分屏能力随共享宿主消亡（workspace/CLAUDE.md 已写死「页内分屏（单页多组）不可用」），销项注记未披露。用户裁决：登记接受，不恢复。

**位置**：`docs/compromises.md:34`

**修复步骤**：CP-004 注记末尾补「；已知功能回退登记：页内分屏（单页多组）随共享宿主消亡——页 = 单组多页签，拖拽拆分产物被回迁守卫清理（2026-09-09 裁决：接受，不恢复；workspace/CLAUDE.md 已写死该形态）」。

**验证**：`rg -n "页内分屏" docs/compromises.md` 命中 CP-004 行。

---

### DOC-06 · CP-005/CP-011 守卫口径回写（review-02 #3/#6 文档面）

**位置**：`docs/compromises.md:48`（CP-011 注记）；`docs/compromises-fix/checklist.md:674`（CP-005 验证节命令）、`:851`（CP-011 验证节命令）

**修复步骤**：

1. compromises.md:48 CP-011 注记「无裸 join 无界阻塞路径」→「生产+测试全仓零裸 join（守卫命令：rg "\.join\(\)" src-tauri/src 仅命中 thread_join.rs 内守卫白名单一处）」。
2. compromises-fix/checklist.md:674（CP-005 验证节）守卫命令改 `rg "std::sync:(Mutex|RwLock)" src-tauri/src -g "*.rs"`（与 BE-04 落定的命令形态逐字一致——注意 md 内正则写作 `std::sync::(Mutex|RwLock)`，照 BE-04 步骤原文）。
3. compromises-fix/checklist.md:851（CP-011 验证节）命令改 `rg "\.join\(\)" src-tauri/src` 仅命中 thread_join.rs 口径。
4. 回写形态同 DOC-04（落地复核注记追加，不改写历史断言原文）。

**验证**：`rg -n "thread_join" docs/compromises.md docs/compromises-fix/checklist.md` ≥ 2；两命令实跑结果与断言一致。

---

### DOC-07 · compromises.md CP-006 注记补 keyset 根治（review-02 #5 文档面）

**位置**：`docs/compromises.md:38`

**修复步骤**：CP-006 注记末尾补「（2026-09-09 复核加固：游标改 keyset——上一页末条目排序键（isDir+小写名）base64，目录中途变长的跨页重复/遗漏根治；前端 opaque 契约零改动；续页失败保留首帧）」。

**验证**：`rg -n "keyset" docs/compromises.md` 命中 CP-006 行。

---

### DOC-08 · compromises.md CP-030 注记补探针副作用登记（review-05 #4 文档面）

**位置**：`docs/compromises.md:107`

**修复步骤**：CP-030 注记末尾补「（2026-09-09 登记：TQ-E-10 探针副作用——多 --spec 定向形态第 2 位 worker 独立应用实例必失焦，探针改首 worker fast-fail + 其余降级 warn（TE-05）；官方形态收窄为单 spec 分次/全量 config 数组，e2e-tests/CLAUDE.md 登记）」。

**验证**：`rg -n "TE-05" docs/compromises.md` 命中 CP-030 行。

---

### DOC-09 · 根 CLAUDE.md 计数失真修正 + L1/L2 串行纪律登记（review-05 #5 + review 附记联动 BE-06）

**位置**：`.claude/CLAUDE.md:77`

**现状**：「（lib_tests = src/lib.rs 显式 test target，721 例等价覆盖 --lib）」——计数已失真（现 825 例且随套件增长）。

**修复步骤**：

1. :77 括注改「（lib_tests = src/lib.rs 显式 test target，等价覆盖 --lib 全量用例）」——去计数（防再漂移）。
2. 同文件「核心原则」节补一条（「隔离优先」条后）：「**全量回归串行执行（BE-06）**：L1（cargo test）与 L2（npm test）禁止并行——CP-007 基准用例（scan.rs scan_bench）对 CPU 负载敏感，并行抢核曾致中位 410ms 误红（2026-09-08 实测，隔离复跑 180ms）；基准已加两轮复判加固，持续并行负载仍不抗」。

**验证**：`rg -n "721" .claude/CLAUDE.md` 零命中；`rg -n "串行" .claude/CLAUDE.md` 命中新条。

---

### DOC-10 · test-exemptions.md:70 键级哨兵口径（review-05 #6）

**位置**：`.claude/test-exemptions.md:70`

**现状**：「exit 时对真实屋做存在性 + sha256 快照比对（任何泄漏独立报红 exitCode=1）」——旧整文件 sha256 口径，实际已改哨兵键级。

**修复步骤**：该句改「exit 时对真实屋做哨兵键级比对（~/.claude/settings.json 的 hooks/statusLine/env 存在性+值快照比对；~/.slterminal/statusline-backup.json 维持文件 sha256、hooks/ 维持整树快照——任何泄漏独立报红 exitCode=1）」。

**验证**：`rg -n "存在性 \+ sha256 快照比对" .claude/test-exemptions.md` 零命中；新表述命中。

---

### DOC-11 · compromises.md CP-012/CP-022/CP-007 注记复核加固补记（SEC-02/FE-03~05/BE-06 联动）

**位置**：`docs/compromises.md:55`（CP-012）、`:84`（CP-022）、`:40`（CP-007）

**修复步骤**：

1. CP-012 注记（:55）末尾补「（2026-09-09 复核加固：预览域宿主页落地域级 CSP meta——default-src 'none' + script/style 'unsafe-inline' + img/font data:，预览内容外部出网通道结构性关闭，SEC-02）」。
2. CP-022 注记（:84）末尾补「（2026-09-09 复核加固：信息条大小改 fs_stat 真实字节（FE-04）、行文本色响应式取色（FE-03）、块缓存文件变更失效重扫（FE-05）、编辑器 10MB 检查前置 stat 零读盘（FE-08））」。
3. CP-007 注记（:40）末尾补「（2026-09-09 复核加固：基准断言改两轮制（越门槛冷却 2s 重采样复判，BE-06）；全量回归 L1/L2 串行纪律登记根 CLAUDE.md）」。

**验证**：三行各 grep「2026-09-09 复核加固」命中。
