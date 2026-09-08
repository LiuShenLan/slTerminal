# Review 01 · 依赖与技术选型（CP-001/002/003/032/033/038）

## 问题清单

### 1. check-ts7-trigger.mjs 不校验 HTTP 状态码，查询失败的退出码 2 语义有洞
- **CP**: CP-001
- **位置**: scripts/check-ts7-trigger.mjs:24-40,42-61
- **问题**: `getJson` 只处理传输层 error 与 JSON 解析失败，不检查 `res.statusCode`。GitHub API 未认证限流（403）或 registry 异常时返回的是**合法 JSON 但无业务字段**的错误体（如 `{"message":"API rate limit exceeded..."}`）——`issue?.state` 为 undefined → 走 `evaluateTrigger` 得 `issueClosed=false` → 退出码 1「未达成」，而非登记语义「退出码 2 = 查询失败（未知态，不误导判定）」。限流场景下脚本会把「查不到」误报为「条件未达成」，与 compromises.md:19 / ADR-0010（adr.md:240）登记的退出码语义不符。
- **证据**: 脚本全文无 `statusCode` 检查；`main()` 的 try/catch 只兜 `getJson` 的 reject（网络/解析），403 JSON 体正常 resolve；退出码分支仅 0/1/2 三态由 `r.triggered` 与查询异常决定。
- **建议**: `getJson` 内对非 2xx 响应 reject（或校验解析结果含预期字段），让查询失败正确落退出码 2。

### 2. check-ts7-trigger.d.mts 声明的入参类型窄于运行期契约
- **CP**: CP-001
- **位置**: scripts/check-ts7-trigger.d.mts:13-16 vs scripts/check-ts7-trigger.mjs:18
- **问题**: 声明文件写 `latestVersion: string | undefined`，而实现以 `latestVersion ?? ""` 显式兜 `null`（dist-tags 缺失时 `registry?.["dist-tags"]?.latest` 可为 undefined，测试用例 `evaluateTrigger_查询失败入参null_不达成` 更是直驱 `null`）。声明比实现窄，后续照声明改实现会把 null 兜底的运行期契约改没。
- **证据**: deps-ts7-trigger.test.ts:35 `evaluateTrigger(undefined, null as unknown as string)` 靠 cast 绕过声明；mjs:18 `?? ""` 兜 null/undefined。
- **建议**: 声明改 `string | null | undefined`，与实现及测试驱动形态对齐。

### 3. compromises.md CP-002 销项括注「jsonSchemaCm.ts 五导出，JsonMode 生产接线」与代码实际不符
- **CP**: CP-002
- **位置**: docs/compromises.md:21 vs src/features/cliProfiles/profiles/claude/configEditor/jsonSchemaCm.ts:43,86,92,112,134,152,158
- **问题**: jsonSchemaCm.ts 实际导出 7 个函数；仅 `hooksSchemaLinter`/`hooksSchemaHover` 2 个被 JsonMode.tsx:154-155 生产接线，其余 5 个（pointerToRange/lintHooksSchemaDoc/resolveSchemaPath/pathAt/resolveHoverDescription）供测试直驱——该文件头注（:11-15）自述了此分层。销项括注的「五导出，JsonMode 生产接线」把数量与接线关系都写错，易误导后续维护者以为五个导出都在生产路径。
- **证据**: grep `^export function` jsonSchemaCm.ts = 7 命中；JsonMode.tsx import 仅 `{ hooksSchemaHover, hooksSchemaLinter }`（:19）。
- **建议**: 括注改为「jsonSchemaCm.ts 七导出（二生产接线 + 五测试直驱）」口径。

### 4. `.temp/node22.bak` 删除条件已满足但残留未清，checklist 步骤 8 未执行
- **CP**: CP-003
- **位置**: .temp/node22.bak（本地，gitignore:28 覆盖）
- **问题**: checklist.md:383 步骤 8 明令「若步骤 1 曾改名 `.temp/node22`，验证通过后**删除** `.temp/node22.bak`」；execution-plan.md S03 备注「.temp/node22.bak 待删」；compromises.md:23 销项注记「待全量 e2e 确认后删」。S12 收尾（6af3791，「全量回归」）已跑完全量 e2e，删除条件即已满足，但 .temp/node22.bak 仍在磁盘（2026-09-08 核查实证）。虽为 gitignored 本地残留且销项注记已披露，但相对 checklist 步骤 8 是未执行项，构成登记在案却未兑现的尾巴。
- **证据**: `ls .temp/` 仅 `node22.bak`（8-16 建）；6af3791 commit body「全量回归全绿」。
- **建议**: 删除 .temp/node22.bak（及 .temp/node22 若存在），并把 compromises.md 销项注记中的「待删」句改为已删口径。

### 5. verify/stage-10.md 声称 CP-033 红测演练「在 commit body 留痕」，实际不存在
- **CP**: CP-033
- **位置**: docs/compromises-fix/workflows/verify/stage-10.md:15
- **问题**: 该断言要求「红测演练记录（临时改产物一字符 → 守卫非 0，还原后 0）在 commit body 留痕」。实查 c6758e5..6af3791 全链 commit body：grep `红测|一字符|守卫非|diff --exit-code|katex` 仅 1 条无关命中（CP-004「全场景演练」）；11bf157 body 只提「CI 守卫（ci.yml CP-033）属本 Stage 交付」，无任何红测演练记录。verify 断言的留痕声称失实。
- **证据**: `git log --format="%h|%b" c6758e5..6af3791 | grep -i "演练\|一字符\|守卫非\|diff --exit-code\|katex"` 输出与声称不符。
- **建议**: 补做一次红测演练并把记录写入 commit body（或修订 verify 断言的留痕位置声称）。

### 6. CP-033「字体真实加载渲染」实证的可复核痕迹不足，现存 L4 断言不承载该声称
- **CP**: CP-033
- **位置**: e2e-tests/markdown.e2e.ts:114-115；.claude/adr.md:450
- **问题**: compromises.md:27 销项声称「data: 字体在预览域真实加载渲染实证关闭原缺口（probe 双轮 + document.fonts.check）」。实证载体是「markdown.e2e 临时用例」（ADR-0018:450 自述），已随 S10 移除、无入库产物；现存常驻 L4 断言只查预览文档含 `data:font/woff2;base64,` 字符串——CSS 串在 head 里不等于字体真实加载（font-family 命中/fonts.check），距「真实加载渲染」声称差一层且无自动化锚点。唯一留痕是 ADR 散文里的 HUD 百分比（121%/110%），不可复跑。
- **证据**: markdown.e2e.ts:115 断言串 `data:font/woff2;base64,`；grep `fonts.check|font-family` e2e-tests/ = 0 命中；ADR-0018:450 全部实证描述为一次性临时用例运行结果。
- **建议**: 把字体加载断言（getComputedStyle font-family 命中 KaTeX 族 + document.fonts.check）固化为常驻 e2e 用例（S10-① 已证预览上下文 execute 可达），替换或补强 :114-115 的串存在断言。

### 7. checklist/verify 的 CP-033 B2 字面断言（「新 webview 局部 CSP 放行 font-src data:」）与实际落地形态不符，执行文档未回写修订
- **CP**: CP-033
- **位置**: docs/compromises-fix/checklist.md:1915；docs/compromises-fix/workflows/verify/stage-10.md:15 vs .claude/adr.md:452,460
- **问题**: checklist B2 预设「在 S10 新 webview 的**局部 CSP** 中显式放行 font-src data:」，verify B2 断言「新 webview 局部 CSP 放行 font-src data:（Read tauri.conf.json / webview 配置确认）」。实际落地（ADR-0018:452/:460 记录）= 预览域**无任何 CSP**（自定义协议响应无 CSP 头），data: 天然放行，根本不存在可 Read 的局部放行配置。偏差方向（无 CSP 域优于局部放行）有 ADR  rationale 记录，但 checklist/verify 作为「修复步骤真值源」未回写修订，后续者按字面执行会发现断言无法满足。
- **证据**: grep `font-src` src-tauri/tauri.conf.json 主窗口已回收 data:（终态 `'self'`）；全仓无预览局部 CSP 配置落点；ADR-0018:460「预览 webview CSP 无代码落点」。
- **建议**: 在 checklist.md/verify/stage-10.md 的 B2 条补一句实际落地口径（无 CSP 域替代局部放行，ADR-0018 维持记录为准）。

## 界外观察

- vitest.config.ts:7 的 exclude 数组含 `'datalearncodeterax-ai-temp'`——明显是某路径片段（`data/learn/code/terax-ai-temp` 形态）被去斜杠拼接的损坏目录名，自 0d6707f（Phase 1）存续至今，与本修复链无关；exclude 意图大概率是 `.temp`（已有相邻项），该坏项从未生效也永不命中，建议顺手清理。
