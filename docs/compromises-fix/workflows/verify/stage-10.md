# Stage 10 逐项验证断言（唯一真值源）

> stage-10 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> Stage 特殊纪律（fix-loop constraints）：spike no-go 时不启动 ②③④，转休眠登记而非强行修复。
> no-go 分支：若 spike 结论为 no-go，本 Stage 唯一断言 = compromises.md 四条（CP-012/013/035/044）+ CP-031/033 补 spike 结论注记转休眠，其余断言全部 N/A。

## 断言清单（go 分支）

- **S10-① spike**：spike 四问（枚举/execute 可达/焦点语义/销毁语义）各有实测答案（yes/no + 证据）落档 `e2e-tests/CLAUDE.md`；加载方式结论（asset vs data:）落档；spike 临时代码不入库（`git status` 无探针残留）。
- **CP-012**：`grep -c "unsafe-inline" src-tauri/tauri.conf.json` = 1（仅剩 style-src）；`grep -c "dangerousDisableAssetCspModification" src-tauri/tauri.conf.json` = 0；`npx vitest run src/__tests__/csp-config.test.ts` 绿（随全量覆盖，含「script-src 恰好等于 ['self']」改写用例）。
- **CP-013**：`grep -rn "slterm_key" src/` = 0；`grep -rn "TRUSTED_MARKER" src/` = 0；上行消息集合恰为 {slterm_zoom, slterm_scroll, slterm_nav}（Read previewMessages.ts 确认）；`command-catalog.test.ts` 守卫已改写为「预览消息通道不含命令重放」形态（随全量覆盖）；L2 负面用例「slterm_key 被静默忽略」存在且通过。
- **CP-031（消亡判定，S10 收尾执行）**：`grep -rn "escapeScriptClose" src/` 退出码 1；新注入机制中宿主 HTML 的 `<script>` 段不经字符串级转义（Read 新架构源码确认）；`grep -n "it.skip" e2e-tests/html.e2e.ts` 相对 S10 前基线 -1；改写用例在 html spec 真实通过（或按事件属性通道改写 + 注释登记，以架构实际能力为准）。
- **CP-044**：收敛分支：`grep -rn '"\*"' src/panels/docViewer/ | grep -v PREVIEW_ORIGIN` = 0；`grep -rn "PREVIEW_ORIGIN" src/panels/docViewer/` ≥4；`npx vitest run doc-viewer-preview-messages doc-viewer-injection` 绿（上行/下行白名单守卫用例）。通道退役分支：spike 产物明确记录「不构成 window postMessage 关系」，本条以通道退役销项且 ADR-0019 登记。
- **CP-033**：A 步（两分支都要）：`grep -n "KaTeX 内联产物 diff 守卫" .github/workflows/ci.yml` 命中；`scripts/gen-katex-inline.mjs` 注释已改 361KB 口径；红测演练记录（临时改产物一字符 → 守卫非 0，还原后 0）在 commit body 留痕。B1 分支：`test -f src/panels/markdown/generated/katexInlineCss.ts` 为假；`grep -rn "gen-katex-inline" scripts/ src/` 零命中；tauri.conf.json font-src 含 `asset:`。B2 分支：主窗口 CSP 不动，新 webview 局部 CSP 放行 `font-src data:`（Read tauri.conf.json / webview 配置确认）；ADR-0018 追加实证结论。（落地复核 2026-09-09：tauri 2.11 无 per-webview CSP 配置面——B2 实际形态 = 宿主页 CSP meta（preview.rs HOST_PAGE）：default-src 'none'; script-src/style-src 'unsafe-inline'; img-src data:; font-src data:，SEC-02 落地；红测演练留痕于 docs/compromises-fix-review-fix Stage 07 commit body）
- **CP-035**：`grep -o "data:" src-tauri/tauri.conf.json` = 0 行（主窗口 CSP 终态）；csp-config.test.ts 三改写 + 「data: 不在主窗口任何指令」新守卫存在（随全量覆盖）；markdown assets.ts MIME 白名单剔除 `image/svg+xml`（Read 确认；若保留则 ADR-0019 有理由登记 + L2 锁白名单断言）；ADR-0019 已立且含 font-src 实证记录归档（③ 产物）。
- **CP-037 复核**：预览迁 webview 后 display:none 保活形态复核结论登记于 ADR-0019（维持/翻案二选一，Read 确认）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run test:l3`
5. `npm run e2e`（html/markdown spec 新 webview 下全绿）
6. `npx tauri build --debug --no-bundle`（tauri.conf.json 变更构建验证）

## 人工验证点

- ① spike go/no-go 结论人工确认。
- 预览行为回归：html/markdown 预览的缩放/滚动/导航全场景人工过一遍。
