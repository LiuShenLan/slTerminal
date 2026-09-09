# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-01**：`rg "b == b':'" src-tauri/src/preview.rs` 命中且同行或相邻行含 `b == b'/'`（Read 确认字符判定两处放行）；`label_accepts_legal_panel_ids` 测试含正例 `preview-page-1:html-2` 与 `preview-a/b`（Read 确认）；`label_rejects_illegal_forms` 不再含 `preview-a/b`（Read 确认）。
- **SEC-01（L4 面）**：`e2e-tests/html.e2e.ts` 含页前缀形态用例（panelId 含 `:` 的 preview label，Read 确认走真实 label 路径而非裸 id）；L4 `node e2e-tests/run-wdio.cjs --spec html.e2e.ts` 绿（测试 agent 结果承载；探针 fast-fail 属环境前提不满足，判 partial 并注明）。
- **SEC-02**：`rg "Content-Security-Policy" src-tauri/src/preview.rs` 命中；Read 确认 HOST_PAGE 的 `<meta charset>` 后插 meta，content 逐字为 `default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:`；L1 用例 `host_page_carries_domain_csp` 存在（rg 命中）。
- **SEC-02（文档面）**：`rg -i "csp" src-tauri/src/CLAUDE.md src/panels/docViewer/CLAUDE.md src/panels/CLAUDE.md .claude/adr.md` 四处各命中（预览域宿主页 CSP meta 登记；Read 抽查一处确认口径 = 宿主页 meta 形态而非 per-webview 配置）。
- **SEC-02（红线面）**：iframe sandbox 属性不含 `allow-same-origin`（rg 确认 PreviewFrame.tsx 现状未变）；主窗口 CSP 配置未被触碰（`git diff` 无 tauri.conf.json / csp-config.test.ts 改动——Read git status 确认）。
- **SEC-03**：`rg "syncWarned" src/panels/docViewer/PreviewFrame.tsx` 命中（sync catch 一次性旗标，且成功路径复位——Read 确认复位逻辑）；close catch 含 `console.warn`（Read 确认）；`src/__tests__/html-panel.test.tsx` 含 sync 一次性 warn + 复位 + close warn 用例（Read 确认），npm test 绿（测试 agent 结果承载）。
- **FE-06**：`src/__tests__/doc-viewer-injection.test.ts` 段组合 describe 内含三组合矩阵用例（无 extra / fragmentNav / linkRouter+scrollReport），各断言 `<script>` 与 `</script>` 计数 toHaveLength(1)（Read 确认三用例存在且断言形态正确），npm test 绿（测试 agent 结果承载）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npm test`
6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
7. `npx tauri build --debug --no-bundle`
8. `node e2e-tests/run-wdio.cjs --spec html.e2e.ts`（前提：窗口前台聚焦）
