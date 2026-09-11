# Review2 · Stage 07 文档登记（DOC-01~11 + 收尾 commit 5b4e12f）

> commit: 75f7a74 + 5b4e12f；核验维度：保真度/实现质量/新引入问题；范围：Stage diff 面 + 触碰文件全量复查。
> 仅登记问题，不分严重度；已核验通过项不列出（DOC-01~11 各 rg 断言、红测演练 commit body 四行留痕、gen-katex 幂等归位、`.temp/node22.bak` 已删、保留项未误改等均已核验通过）。

## 问题清单

### R2-DOC-01 · 收尾 commit「全仓修正 11 处」失实——现役文件同义失实表述漏改 5 处

- **关联修复项**: SEC-02（收尾 commit 5b4e12f）
- **位置**: `src/__tests__/markdown-assets.test.ts:19`（「预览域（无 CSP）内联风险面大」——同句在 markdown/CLAUDE.md:28 已改、本处漏改）；`e2e-tests/html.e2e.ts:163`（「宿主页域（自定义协议）无全局 CSP」）；`e2e-tests/html.e2e.ts:293`（「（自定义协议宿主页 iframe，无全局 CSP）」）；`e2e-tests/markdown.e2e.ts:14`（「无 CSP 下同样成立」）；`e2e-tests/markdown.e2e.ts:234`（「预览域无 CSP——onerror 正常执行」）
- **问题**: SEC-02 落地后预览域已有域级 CSP meta（default-src 'none' + 内联 script/style + data: img/font），上述 5 处注释仍以「无 CSP/无全局 CSP」描述预览域，与 5b4e12f 已修正的 11 处口径矛盾；commit message「失实表述全仓修正 11 处」的「全仓」声明不成立。
- **证据**: `rg -n "无 CSP|无全局 CSP|域内无 CSP|零 CSP|域无 CSP"` 全仓实跑，除已修正/事实陈述保留项外仍命中上述 5 处现役注释。
- **建议**: 按 5b4e12f 同款口径（「CSP meta 放行内联脚本/img/font data:」）补齐 5 处注释修订。

### R2-DOC-02 · adr.md/test-exemptions.md 历史区现在时「预览域无 CSP」表述无复核注记指引（4 处）

- **关联修复项**: SEC-02（收尾 commit 5b4e12f）
- **位置**: `.claude/adr.md:446`（「在预览域（无 CSP）渲染」）、`:459`（「预览域（无 CSP）内联风险面大」）、`:505`（「预览域维持无 CSP（data: img/font 天然放行，无代码落点）」——CP-035 结果登记现在时）；`.claude/test-exemptions.md:39`（「事件属性通道实证保留（预览域无 CSP）」）
- **问题**: ADR 历史记录保留原文语义可立，但上述表述以现在时陈述且已失实；adr.md 仅 :460-461 有 SEC-02 落地复核注记（且注记声明只覆盖「上条」），446/459/505 三处与 test-exemptions.md:39 无任何指引，读者无法从原文得知表述已失效。
- **证据**: 实读四处原文 + adr.md:461 注记覆盖范围（仅紧邻上条）。
- **建议**: 参照 :461 形态为其余各处补交叉指引注记（或统一在 ADR-0018/0019 节首加一句「SEC-02 后预览域 CSP 表述以 :461 注记为准」）。

### R2-DOC-03 · execution-plan.md 进度表计数自相矛盾（8 处 vs 11 处）

- **关联修复项**: —（存量/收尾文档）
- **位置**: `docs/compromises-fix-review-fix/execution-plan.md:84`（row 04「失实表述残留 **8 处**」——但其列举 injectScript.ts:11、PreviewFrame.tsx:13、assets.ts:12、markdown/CLAUDE.md:28/29/52、csp-config.test.ts:4/93/110、html-panel.test.tsx:362 实为 **10 个点位**）vs `:87`（row 07「修正 **11 处**」，含 preview.rs:443）
- **问题**: 同一进度表内同一事项的计数三个口径（8/10/11）互不一致；历史执行档虽不改写，但矛盾计数未加注记，后续追溯易误读。
- **证据**: 实读两行原文比对；5b4e12f diff 实际修正 11 处（含 preview.rs:443，row 04 列举未含）。
- **建议**: row 04 或 row 07 补一句注记说明计数口径差异（历史原文不动）。

### R2-DOC-04 · verify/stage-07.md DOC-02 断言「.temp/node22 预置通道仍在」与常态不符

- **关联修复项**: DOC-02
- **位置**: `docs/compromises-fix-review-fix/workflows/verify/stage-07.md:9`（「`.temp/node22` 预置通道仍在（Test-Path 确认未误删存活功能）」）
- **问题**: `.temp/node22` 是按需显式预置目录（run-wdio.cjs:326-336：存在且 >1MB 才启用便携 Node 22，否则走系统 Node）——主 toolchain Node≥22 直跑的常态下该目录本就不存在，本轮实测 MISSING。断言按字面执行恒假，会误导后续复核者误判「误删存活功能」。
- **证据**: `Test-Path .temp/node22` 实跑不存在；run-wdio.cjs:326-336 实读（存在性可选分支）；CP-003 销项（自动下载分支删除、Node 直跑）。
- **建议**: 该断言改写为「run-wdio.cjs 的 .temp/node22 预置分支代码仍在（rg 命中），目录本身按需预置、不存在属常态」。
