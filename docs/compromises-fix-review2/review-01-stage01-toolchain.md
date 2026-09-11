# Review2 · Stage 01 工具链与门禁（TE-01/02/03/07）

> commit: 7abc009（注：任务给定 hash 37b7b50 在仓内不存在，Stage 01 实际 commit = 7abc009，按同一范围核验）；核验维度：保真度/实现质量/新引入问题；范围：Stage diff 面 + 触碰文件全量复查。
> 仅登记问题，不分严重度；已核验通过项不列出。

## 已核验通过（摘要，不计问题）

- 保真度：TE-01/02/03/07 实现与 checklist 步骤逐字一致；verify/stage-01.md 断言逐条成立（rg 六组断言实跑全过；d.mts 双参 `string | null | undefined`；ci.yml:67 step 两条命令逐字在位；双 CLAUDE.md 各恰一处「CI 门禁 step 已落地」）；commit message 与 stages.md 约定逐字一致。
- 定向实跑全绿：`npx vitest run src/__tests__/deps-ts7-trigger.test.ts` 7/7 过；`cargo test --test lib_tests export_bindings` 23 passed；`git diff --exit-code -- src/types` exit 0；`npx tsc --noEmit` exit 0；`npx eslint src/__tests__/deps-ts7-trigger.test.ts` exit 0；`npx knip --production` exit 0（getJson 新导出经 scripts/** entry 不产生死导出新红）；`node scripts/check-ts7-trigger.mjs` 退出码 1（实网未达成，∈{0,1,2}）。工作树核验后干净。

## 问题清单

### R2-TE-01 · main() 查询失败日志标签未随非 2xx 语义同步
- **关联修复项**: TE-01
- **位置**: scripts/check-ts7-trigger.mjs:57
- **问题**: TE-01 将「非 2xx 响应」纳入「查询失败 → 退出码 2」语义，脚本头注（:6）已同步「（含非 2xx 响应——TE-01）」，但运行期错误消息仍打印 `查询失败(网络/解析):${e.message}`——非 2xx 落码 2 时实际输出形如 `查询失败(网络/解析):HTTP 403」，分类标签「网络/解析」不覆盖 HTTP 状态码失败面，与头注新语义表述不一致。checklist TE-01 步骤 4 只写死头注同步，故实现与 checklist 逐字一致；此条为触碰文件全量复查发现的关联缺口。
- **证据**: 实读 scripts/check-ts7-trigger.mjs:6（头注已含「含非 2xx 响应——TE-01」）vs :57（`console.error(`[ts7-trigger] 查询失败(网络/解析):${e.message}`)`）；:24-36 守卫 reject `new Error(`HTTP ${status}`)` 即此标签覆盖不到的失败类。
- **建议**: 标签泛化为「查询失败(未知态)」或补「/HTTP 状态码异常」，与头注口径对齐。

### R2-TE-02 · vitest.config.ts exclude 整段为死配置且覆盖默认防护语义
- **关联修复项**: —（存量问题；TE-07 仅删损坏项，剩余三条目同为无效配置）
- **位置**: vitest.config.ts:7
- **问题**: `include` 已限定 `src/__tests__/**/*.test.ts(x)`，exclude 三个条目（`'node_modules'`、`'.temp'`、`'e2e-tests'`）不可能匹配任何 include 命中文件——纯死配置。且 vitest 自定义 exclude 会**整体覆盖**默认 exclude（默认含 `**/node_modules/**`），裸字符串 `'node_modules'` 无 glob 语义，若 `src/__tests__` 下出现嵌套 `node_modules`（如夹具内嵌依赖树）中的 `.test.ts`，默认防护会漏。TE-07 清掉损坏项后该面维持原状。
- **证据**: 实读 vitest.config.ts:6-7——include 前缀 `src/__tests__/` 与 exclude 三条目零交集；picomatch 裸串不匹配嵌套路径。
- **建议**: 整段删 exclude（include 自足），或恢复 vitest 默认形态并只追加确需项。
