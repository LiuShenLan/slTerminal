# Review2 · Stage 02 预览链路（SEC-01/02/03+FE-06）

> commit: d1a6838（任务所给 47909e1 为笔误，git 无此 revision；主题行与 Stage 02 完全对应）；核验维度：保真度/实现质量/新引入问题；范围：Stage diff 面 + 触碰文件全量复查。
> 仅登记问题，不分严重度；已核验通过项不列出。

定向实跑证据：
- `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests preview -- --test-threads=1` → 12 passed 0 failed（含 host_page_carries_domain_csp）
- `npx vitest run src/__tests__/html-panel.test.tsx src/__tests__/doc-viewer-injection.test.ts` → 2 files / 62 tests 全绿
- CSP 指令表逐字核对：preview.rs:456 与 checklist :86 逐字一致（default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:）
- `rg "preview-a/b" src-tauri/src/preview.rs` → 仅 :581 合法集一处；反例集已无
- tauri-runtime-2.11.3 window.rs is_label_valid 声称属实（alphanumeric + - / : _）
- 主窗口 CSP 配置零触碰（git show d1a6838 --stat 无 tauri.conf.json / csp-config.test.ts）

## 问题清单

### R2-SEC-01 · host_page_carries_domain_csp 断言面未覆盖 script-src/style-src 'unsafe-inline'
- **关联修复项**: SEC-02
- **位置**: src-tauri/src/preview.rs:604-611
- **问题**: L1 用例只断言 Content-Security-Policy / default-src 'none' / img-src data: / font-src data: 四项，script-src 与 style-src 的 'unsafe-inline' 放行面无 L1 锁。若该两指令被删/收窄（如 script-src 回退 default-src 'none'），宿主页桥内联脚本与注入产物全灭——L1 不红，需 L4 html.e2e 缩放用例才暴露。checklist 步骤 3 写死的即为这四断言（保真度合规），但按「断言是否真锁住行为」维度属弱覆盖。
- **证据**: preview.rs:606-610 四条断言无 script-src/style-src；实测跑绿仅证四指令存在。
- **建议**: 用例内补两条 contains 断言（script-src/style-src 'unsafe-inline'），与 checklist 指令表全量对齐。

### R2-SEC-02 · checklist SEC-02 写死的 L4 双 spec 验证未完整执行（markdown.e2e.ts 缺）
- **关联修复项**: SEC-02
- **位置**: docs/compromises-fix-review-fix/checklist.md:102；docs/compromises-fix-review-fix/execution-plan.md:82；commit d1a6838 body
- **问题**: checklist SEC-02 验证写死「`--spec html.e2e.ts` + `--spec markdown.e2e.ts` 全绿」（markdown 面 = data: 图/KaTeX 字体链路在 CSP 下不回归）。Stage 02 实际只跑 html.e2e.ts（commit body 与 execution-plan :82 登记一致，均无 markdown 留痕）。markdown 链路 CSP 下验证由 S06 全量 e2e（857dca0 body：npm run e2e 15 passed，含 markdown.e2e.ts KaTeX_Main + fontProbe 断言）间接覆盖——行为面有终态证据，但 Stage 02 自身 verify 清单未完整执行且未登记该偏差。
- **证据**: checklist :102 双 spec 要求；d1a6838 body 仅「--spec html.e2e.ts 绿（1 spec passed）」；execution-plan :82 仅 html。
- **建议**: 无代码动作；execution-plan :82 备注补一句 markdown 面由 S06 全量覆盖即可（或维持现状，事实已在 S06 留痕）。

### R2-SEC-03 · verify/checklist「catch(() => 零命中」rg 断言字面不成立
- **关联修复项**: SEC-03
- **位置**: docs/compromises-fix-review-fix/workflows/verify/stage-02.md:13；docs/compromises-fix-review-fix/checklist.md:160；src/panels/docViewer/PreviewFrame.tsx:182
- **问题**: verify 断言与 checklist 验证均写死 `rg -n "catch\(\(\) =>" src/panels/docViewer/PreviewFrame.tsx` 零命中（空参静默 catch 形态绝迹）。实跑该 rg 恒命中 :182——previewRender 的 `catch(() => {` 空参形态（体内有 console.warn，非静默；checklist SEC-03 步骤 3 明确 render 路径不动，:182 为 S10-② 存量）。「零命中」在此文件上不可达；执行侧（fix-loop 首轮 allFixed=false 的 2 项 partial）未留痕说明该断言如何判定。断言真值源写死了不可达条件。
- **证据**: 实跑 `rg "catch\(\(\) =>" src/panels/docViewer/PreviewFrame.tsx` → 命中 182:`void previewRender(label, srcDoc, iframeBg).catch(() => {`（非静默，:184 有 console.warn）。
- **建议**: verify/checklist 断言精确化为「sync/close 两处空参静默 catch 绝迹」或 rg 模式排除 render 行。

### R2-SEC-04 · PreviewFrame「下轮轮询自愈」注释失实（轮询早退不重发 sync）
- **关联修复项**: SEC-03
- **位置**: src/panels/docViewer/PreviewFrame.tsx:205、:242
- **问题**: 注释称「下轮轮询自愈」「下轮成功即复位」——但 syncNow 的失败轮会把当轮几何写入 lastX/lastY/lastW/lastH/lastVis（:226-230，调用前更新），几何不变时 200ms 轮询走等值早退分支（:223-225）不再重发 previewSync；真实重试触发源是几何变化或主窗移动事件（moved 合并 setTimeout），不是「下轮轮询」。注释夸大了轮询自愈面，与既有早退语义（亚像素抖动抑制）冲突，易误导后续维护者以为失败会轮询自动重试。
- **证据**: PreviewFrame.tsx:223-230 早退分支在 previewSync 调用前更新快照变量；:250-256 轮询仅调 syncNow（无强制重发路径）。
- **建议**: 注释改为「几何变化/主窗移动触发的下一轮 sync 自愈」。
