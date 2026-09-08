# Review 03 · 安全放宽（CP-012/013/014/015/035/043/044）

## 问题清单

### 1. S11 页前缀 panelId 含 ":" 与 preview validate_label 字符集硬冲突——生产预览链路整体被静默拒绝
- **CP**: CP-012（交叉 CP-013/044，同链路面）
- **位置**: `src-tauri/src/preview.rs:52-68`（validate_label 字符集）、`src-tauri/src/preview.rs:290/345/386/417`（四命令入口全过 validate_label）、`src/workspace/pageGroups.ts:29-30`（panelId = `{pageId}:{localId}`）、`src/workspace/openFile.ts:100,112`、`src/panels/html/HtmlPanel.tsx:257`、`src/panels/markdown/MarkdownPanel.tsx:417`、`src/ipc/preview.ts:46-48`（makePreviewLabel 原样拼接）、`src/panels/docViewer/PreviewFrame.tsx:129,224-226,243-245,178-181`（错误静默吞）
- **问题**: S11 把全部面板 id 改为 `{pageId}:{localId}` 形态，":" 落进 preview label（`preview-<panelId>`）；S10 落地的 `validate_label` 仅放行 ASCII 字母数字/`_`/`-`，":" 硬拒。preview_sync / preview_close / preview_render（store_render_content）/ preview_pull（pull_stored_content）四个入口全部先过 validate_label → 生产路径（openFile 打开 html/md → 面板 params.panelId → PreviewFrame → label）上预览建窗、内容存储、拉取全部 `Err(Validation)`；前端三处调用 catch 全部静默（sync/close 空 catch、render 仅 console.warn）→ 生产预览无声消失。与 review-02-backend.md 问题 1 为同一冲突，此处独立复核确认存在（行号与链路逐跳核实一致）。
- **证据**: ① pageGroups.ts:29-30 `panelIdInPage` 产出含 ":" id；openFile.ts:100/112 该 id 入 params.panelId；HtmlPanel.tsx:257 / MarkdownPanel.tsx:417 原样传 PreviewFrame；preview.ts:46 直接拼接无前段安全化；preview.rs:61-64 `rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')` 对 ":" 恒 false。② L4 全绿掩盖：html.e2e.ts:109/196/329/368 全部用裸 id（`"e2e-html-" + Date.now()`，无页前缀）经 `__dockviewApi.addPanel` 直注，label 天然无 ":"（已实读 :329-337）；生产形态（Explorer 双击打开文件）无任何 e2e/L2 覆盖。③ validate_label 行为由 preview.rs:569-591 用例锁死（含 "preview-a/b" 拒绝），但拒绝集无 ":" 专项用例，S11 页前缀形态无任一侧测试守卫。
- **建议**: makePreviewLabel 单点加安全化（`[^a-zA-Z0-9_-]` 替换为 `_`，与 slterm-hook-reporter 信号文件名安全化同形态）或 validate_label 放开 ":"；补生产形态（页前缀 id）L1/L2 契约用例。

### 2. 预览域零 CSP 引入「预览内容任意出网」新通道——修复新造的安全放宽未登记
- **CP**: CP-012（ADR-0019 预览迁移的直接产物）
- **位置**: `src-tauri/src/preview.rs:449-458`（HOST_PAGE 无 CSP meta）、`src-tauri/src/preview.rs:543-561`（host_protocol 响应仅 content-type，无 CSP 头）、`src-tauri/src/preview.rs:460`（iframe `sandbox="allow-scripts"`，无网络限制旗标）；对照 `git show 11bf157^:src-tauri/tauri.conf.json`（S10 前主窗口 CSP：`default-src 'self'; img-src 'self' data: asset: …`）；`docs/compromises-fix/checklist.md:1932`（S10-④ 计划形态 =「预览 webview CSP 独立放行 data:（img/font）」）；`.claude/adr.md:460/500`（登记「预览域无 CSP」为机制事实与已知行为，但未登记出网面 widening）
- **问题**: 旧架构 srcdoc iframe 继承主窗口 CSP（compromises.md CP-012 条目原文「srcdoc iframe 继承父 CSP(W3C 行为)」）——外部 img/connect 被 `img-src 'self' data: asset:` / `default-src 'self'` 拦住。新架构预览域（自定义协议宿主页 + sandbox iframe）完全没有 CSP：sandbox 仅 allow-scripts（不含网络阻断类旗标），srcdoc 文档从宿主继承的 CSP 为空 → 项目内不可信/含跟踪像素的 html/md 预览内容现可经 `<img src="https://…">`/fetch/beacon 任意出网（外渗本地信息或纯追踪）。checklist S10-④ 步骤 2 的计划形态是「预览域局部 CSP 放行 img/font data:」（其余约束保留），因 tauri 2.11 无 per-webview CSP（spike 实证）实际交付为整域无 CSP——比计划形态宽，且该偏差仅在 ADR 作为「无代码落点」陈述，未在 compromises.md 登记为新妥协/新债务。ADR-0017「本地文件全信任」模型可覆盖脚本执行面，但不覆盖「可信内容里的外部请求」这一网络约束回退。
- **证据**: ① preview.rs:549-554 响应 builder 只设 status/content-type；HOST_PAGE（:449-539）head 内仅 `<meta charset>`，无 CSP meta。② 旧 CSP 字面（git show 11bf157^）：`default-src 'self'; … img-src 'self' data: asset: https://asset.localhost`——srcdoc 子资源出网在旧架构被默认拦截。③ checklist.md:1932 计划「预览 webview CSP 独立放行 data:（img/font）」↔ 交付零 CSP（adr.md:460「预览域当前无局部 CSP……无代码落点」）——计划-交付偏差无登记。④ compromises.md CP-012 销项注记与 ADR-0019「已知行为登记」（adr.md:500）均未含出网面字样。
- **建议**: HOST_PAGE 头加 CSP meta（`default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:` 形态——srcdoc 继承该策略，内联注入与 data: 资源照跑、外部请求全断），或 compromises.md 补登「预览域任意出网」为新妥协。

### 3. exportContextBindings 生产零消费残留 + shortcuts/CLAUDE.md 过时描述——CP-013 退役面未清干净
- **CP**: CP-013
- **位置**: `src/features/shortcuts/ShortcutRegistry.ts:156`、`src/features/shortcuts/types.ts:67,94`、`src/features/shortcuts/CLAUDE.md:62`、`src/__tests__/html-panel.test.tsx:90`、`src/__tests__/markdown-panel.test.tsx:102`
- **问题**: CP-013 把 slterm_key 键盘转发/global 命令重放通道整体退役（销项注记「slterm_key/TRUSTED_MARKER 删除」属实，src/ 零命中已核），但该通道的唯一生产消费方 `ShortcutRegistry.exportContextBindings`（连同 types.ts 的 `ExportedBinding` 导出接口）仍留在生产代码里，生产消费点为零（仅剩测试 mock 与 L2 用例引用其存在）。types.ts:67 注释自述「供未来 iframe 转发脚本用」——正是已退役的转发脚本；shortcuts/CLAUDE.md:62 仍描述其为「供 HtmlPanel postMessage 键盘转发动态比对全局快捷键用」，与退役后事实不符（同文件 :116 已写退役口径，:62 未同步）。属退役面清扫遗漏的死代码 + 双源漂移文档，非安全洞但违背「未来最优」与代码自证纪律。
- **证据**: 全仓 grep `exportContextBindings` 生产引用 = ShortcutRegistry.ts:156（定义）+ types.ts:67/94（类型）+ CLAUDE.md/测试文件，零组件/hook 消费；HtmlPanel/MarkdownPanel/PreviewFrame 均不引用（S10 后注入脚本零键上行）。销项注记只声称消息面删除，未覆盖该 API 的存废判定。
- **建议**: 删除 exportContextBindings/ExportedBinding 及配套测试 mock 引用，或登记保留理由；顺手修 CLAUDE.md:62 描述。

## 各 CP 保真度核实结论（无问题项的验证摘要，供归档）

以下声称经一手核实属实，不构成问题：

- **CP-012**: tauri.conf.json:25 `script-src 'self'`（无 unsafe-inline）、`dangerousDisableAssetCspModification` 键已删（全文件 grep 零命中）；csp-config.test.ts 四守卫（script-src 恰好 ['self']/键不存在/img-src 恰好三项/font-src 恰好 ['self']）+ 「data: 不在主窗口任何指令」全指令守卫存在且实跑 38 例全绿。
- **CP-013**: `slterm_key`/`TRUSTED_MARKER`/`escapeScriptClose` src/ 零命中；上行终态集合 = {slterm_zoom, slterm_scroll, slterm_nav}（previewMessages.ts UPLINK_MSG_TYPES + doc-viewer-preview-messages.test.ts 白名单恰好断言）；PreviewFrame 校验链 label 归属（:263）→ nonce（:269/:280/:288）→ 数值守卫（isFiniteZoom/isFiniteRatio）逐条在码；伪造 nonce/异 label 负面用例在 html-panel.test.tsx:674-708。
- **CP-014**: shell.rs:200-239 三分支实现与 checklist 一致（双成功精确比较/双失败句柄级身份/单侧拒绝）；fallback_identity_match 双侧证据齐且相等才放行，任一侧 None 即拒绝，字符串永不构成证据；file_identity（普通打开跟随 reparse）+ reparse_entry_identity（FILE_FLAG_OPEN_REPARSE_POINT 条目身份）补证据链口径一致；实跑 file_identity×3、paths_match×3、fallback_both_unopenable_rejected 全绿。
- **CP-015**: uninstall_impl（inject.rs:736-812）read/parse 失败 `?` 传播于目录删除（:797-809）之前——「目录全保留」属实；启动对账 reconcile_hooks_on_startup（hooks/mod.rs:233-243，lib.rs:108 挂载）先 ensure 补脚本后 reinject；hooks/CLAUDE.md「非法 JSON 中止」契约行同步；实跑 uninstall_impl_illegal_json_keeps_dirs_and_errors、reconcile_hooks_on_startup_restores_deleted_scripts 全绿。
- **CP-035**: 主窗口 CSP 无 data:（grep -o "data:" = 0）；assets.ts MIME 白名单剔除 image/svg+xml（markdown-assets.test.ts 锁死）；主窗口唯一 data: 消费点 CM6 lint 波浪线已改 text-decoration 技法（theme/overrides.ts:60-87 代码面无 data: url，仅注释提及）；ADR-0018「回收记录」+ ADR-0019 结果登记齐备。
- **CP-043**: audit_suspicious_statusline 返 Option + audit 通道（inject.rs:163-172）；inject_impl 审查闸在备份写/原子写回之前（:462-475），pending 路径 settings.json 零写盘（脚本已落盘但无 matcher 引用惰性，注释登记）；reinject 路径命中即跳过+审计（:637-639）；confirm_inject 三处注册（lib.rs/build.rs/capabilities 均命中）；DTO 第四态 + suspicious_command skip_serializing_if 契约（hooks/mod.rs:33-64）；前端确认条确认/取消双路（ClaudeHooksConfigEditor.tsx:482-549），取消纯清 state 零写盘；实跑 suspicious×7、pends_confirmation、uninstall 相关全绿。TOCTOU 面无（confirm 经 inject_impl 重读 settings，二次调用幂等——桥接已存在即跳过分支 :438-441）。
- **CP-044**: 跨窗 postMessage 通道退役属实（spike 结论登记 ADR-0019 决策一.2）；消息桥 = Tauri event，上行 label 由宿主页从 `T.metadata.currentWebview.label` 自取（preview.rs:465,510）不可被 iframe 伪造（iframe opaque + 无 IPC 注入 main_frame_only）；主窗 PreviewFrame label 归属 + 类型白名单 + nonce + 数值四重校验；iframe 侧 zoomRuntime/scrollRuntime 下行 source===parent + nonce + type 三重校验（zoomRuntime.ts:86-95、scrollRuntime.ts:40-47）；窗口树内残留 postMessage 的 "*" 经宿主页 source===iframe.contentWindow + origin "null" 校验兜底，与登记口径一致。

## 界外观察

- 无（本次审查范围内偶发现象均已归入问题清单或上方核实摘要）。
