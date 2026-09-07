// =====================================================================
// Stage 10 预览通道根治（串行为主：① spike → ② 迁移家族 → ③ CP-033 → ④ CP-035）
// fix-loop constraints 取值唯一真值源 = execution-plan.md「fix-loop 调用规范」表 S10 行
//（本脚本 PREAMBLE_EXTRA 已内含 spike no-go 纪律，与表值同源）
// =====================================================================

export const meta = {
  name: 'stage-10-preview-webview',
  description: 'S10 预览通道根治（spike + CP-012/013/031/044/033/035）',
  phases: [
    { title: '① spike 可达性' },
    { title: '② webview 迁移' },
    { title: '③ KaTeX 重实证' },
    { title: '④ 主窗口 CSP 回收' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
【Stage 特殊纪律】spike no-go 时不启动 ②③④，转休眠登记而非强行修复。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md S10 各条目（先读再动手，照抄步骤不另起方向）。`

// === Phase 1: spike（go/no-go 硬前置）===
phase('① spike 可达性')
const spikeResult = await agent(`${PREAMBLE}

你负责 S10-① 多 webview WDIO 可达性 spike（产物 = 结论登记，临时代码不入库）：
照抄 checklist S10-① 步骤 1-4：最小 spike（主窗口 + 独立预览 webview）验证四问：① webview 可枚举；② browser.execute 在预览 webview 上下文可达；③ 焦点语义；④ 销毁语义（关预览后 driver 不挂）。同步实测加载方式两候选（asset 协议宿主页 优先 vs data: 注入）。
产物：go/no-go 结论 + 加载方式结论 + WDIO selector 可达策略，写入 e2e-tests/CLAUDE.md 与 ADR-0019 草稿（adr.md 新立节）。
no-go 出口：S10 整体转同态维持——不迁移；docs/compromises.md 的 CP-012/013/035/044/031/033 各补 spike 结论注记转休眠，Stage 到此为止。
spike 临时代码跑完即删，不进 git。
完成后报告：四问各自的 yes/no + 证据 + 加载方式结论 + go/no-go 总结论（结论必须在报告首行明示）。`, { label: 's10-spike' })

// === Phase 2: ② webview 迁移家族（单 agent 同卡；spike no-go 时本 agent 只核验登记）===
phase('② webview 迁移')
const migrateResult = await agent(`${PREAMBLE}

spike 结论（首行明示 go/no-go）：
---
${spikeResult ?? '（spike agent 未返回——按 no-go 处理）'}
---
若 no-go：核验 compromises.md 六条注记已落，报告「N/A（no-go）」，不做任何代码改动。
若 go：你负责 S10-②（CP-012+CP-013+CP-031+CP-044 同卡，波及面四区显式覆盖）：
1. 先复核 CP-037 保活形态（S07 display:none 恒挂载在预览迁出 dockview 后是否仍适用），结论写入 ADR-0019。
2. webview 迁移契约（checklist CP-012 步骤 1）：docViewer 面板预览内容迁独立 Tauri webview + 专用宿主页；注入机制（injectScript/buildInjectedScript/nonce）原样迁入预览 CSP 域。
3. 消息桥契约（以 spike 结论为准）：上行仅 {slterm_zoom, slterm_scroll, slterm_nav}，下行 reset/zoom_set/scroll_set，无命令重放。
4. CP-012：tauri.conf.json 主窗口 CSP 终态（照抄 checklist 代码块：script-src 'self'，删 dangerousDisableAssetCspModification 整键；img/font data: 本轮不动）。
5. CP-013：删 buildInjectedScript keydown 转发段 + PreviewFrame slterm_key 整分支 + TRUSTED_MARKER（ShortcutRegistry 零消费已实证，注释与实现一并清）。
6. CP-031 消亡判定三条（此时记录中间态，最终断言在 ④ 后收尾执行）。
7. CP-044：迁移期 "*" 加锁 + 上行/下行白名单守卫；落地后 PREVIEW_ORIGIN 单点收敛（previewMessages.ts，终值以 spike 实测 origin 为准）或通道退役（spike 结论为准），两分支口径照 checklist。
8. 波及面四区：shortcuts 重放通道退役 / docViewer 层六件 + 六测试改写 / panelRegistry always 白名单复核 + layoutSerde 存量面板迁移 / html.e2e.ts(:12-80 改写 + :87 skip 用例按 CP-031 口径改写) + markdown.e2e.ts 适配。
测试同步按 CP-012/013/044 三条目逐一点名（csp-config.test.ts script-src 两守卫改写、command-catalog 守卫改写、html-panel/markdown-panel/doc-viewer-injection/doc-viewer-zoom-runtime/doc-viewer-preview-messages/doc-viewer-floating-area 适配）。
文档：panels/CLAUDE.md:164 红线改写；docViewer/CLAUDE.md PreviewFrame 节重写；ADR-0019 正式落（动机/spike 结论/消息桥契约/波及面/CP-037 复核结论）；ADR-0009 SEC-09 行尾追加「已被 ADR-0019 取代」。
完成后报告：修改文件清单 + CP-031 三条消亡判定中间态 + CP-044 走的分支。`, { label: 's10-migrate' })

// === Phase 3: ③ CP-033（② 完成后；no-go 时 N/A）===
phase('③ KaTeX 重实证')
const katexResult = await agent(`${PREAMBLE}

② 迁移报告（含 go/no-go 与 PREVIEW_ORIGIN 分支）：
---
${migrateResult ?? '（未返回——按 no-go 处理）'}
---
若 no-go：报告「N/A（no-go）」，不改动。
若 go：你负责 CP-033：
A. 维持期 CI diff 守卫（必做）：.github/workflows/ci.yml 在 knip 步骤后插入 KaTeX diff 守卫步骤（照抄 checklist yaml 块）；scripts/gen-katex-inline.mjs:8 注释「~0.9MB」改「产物约 361KB（2026-09-06 实测）」；红测演练（临时改产物一字符 → 守卫非 0 → 还原后 0），演练记录写入报告。
B. 新 webview 上下文重实证：真实 WebView2 渲染含数学公式 md（$x^2$ + 块级），断言公式 font-family 命中 KaTeX 字体族且网络面板无字体请求失败/CORS 拒绝（新增临时 L4 spec 或 markdown.e2e.ts 守卫用例）。
B1（asset 通道可行）→ 删生成物与 gen 脚本，mdPipeline 改 asset 协议引用，font-src 增 asset: https://asset.localhost；B2（仍不可行）→ 保留生成物，新 webview 局部 CSP 放行 font-src data:（主窗口不动）。两分支口径照 checklist。
文档：markdown/CLAUDE.md:23 补 CI 守卫句；ADR-0018 追加「逆转记录」或「维持记录」。
完成后报告：修改文件清单 + B 分支结论 + 实证证据。`, { label: 's10-katex' })

// === Phase 4: ④ CP-035（③ 实证为 font-src 回收硬前置；no-go 时 N/A）===
phase('④ 主窗口 CSP 回收')
const cspResult = await agent(`${PREAMBLE}

③ 实证报告（含 B 分支结论——font-src 回收硬前置）：
---
${katexResult ?? '（未返回——按 no-go 处理）'}
---
若 no-go：报告「N/A（no-go）」，不改动。
若 go：你负责 CP-035 + CP-031 消亡判定收尾：
1. tauri.conf.json 主窗口 CSP 终态整串替换（照抄 checklist 代码块：img-src/font-src 双收 data:；style-src 'unsafe-inline' 保留）。【前置闸】③ 实证未通过则 font-src 回收拆为后续项登记 docs/compromises.md，img-src 回收照常。
2. 预览 webview CSP 独立放行 data:（img/font，仅预览域）。
3. svg data: 默认禁用：markdown assets.ts MIME 白名单剔除 image/svg+xml，html 资源通道同口径；若要保留须在 ADR-0019 登记理由 + L2 锁白名单断言。
4. CP-031 收尾三条消亡判定（checklist CP-031 步骤 3）：escapeScriptClose 零命中 / 宿主 script 不经字符串转义 / 预览宿主 script 真实可执行（e2e 断言或事件属性通道改写）——全中才销项；同态维持则翻案出口登记。
测试：csp-config.test.ts img-src/font-src 守卫改写 + 「data: 不在主窗口任何指令」新守卫；markdown 资源 L2 svg 白名单断言。
文档：panels/CLAUDE.md:32 ADR-0018 句改写；markdown/CLAUDE.md 本地资源节登记 svg 禁用；ADR-0018「逆转触发点」节标注已回收；ADR-0019 归档 font-src 实证记录。
完成后报告：修改文件清单 + CP-031 三条判定结果 + svg 处置（禁用/保留+理由）。`, { label: 's10-csp-final' })

// === Phase 5: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npm run test:l3
5. npm run e2e
6. npx tauri build --debug --no-bundle
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 6: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 10 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-10.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
各波次报告（go/no-go 与分支结论出处）：
---
spike：${spikeResult ?? '（未返回）'}
② 迁移：${migrateResult ?? '（未返回）'}
③ KaTeX：${katexResult ?? '（未返回）'}
④ CSP：${cspResult ?? '（未返回）'}
---
若 spike 为 no-go：只核验 verify 文件「no-go 分支」断言（六条转休眠注记），其余断言标 N/A。
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

return { spikeResult, migrateResult, katexResult, cspResult, testResult, verifyResult }
