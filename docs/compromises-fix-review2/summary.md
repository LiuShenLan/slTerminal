# 修复计划（compromises-fix-review-fix）开发结果 Review 汇总

> 分文件：`review-01-stage01-toolchain.md`（S01 工具链门禁）、`review-02-stage02-preview-chain.md`（S02 预览链路）、`review-03-stage03-backend-hardening.md`（S03 后端加固）、`review-04-stage04-frontend-cleanup.md`（S04 前端清理）、`review-05-stage05-large-file-chain.md`（S05 大文件链路）、`review-06-stage06-e2e-infra.md`（S06 e2e 设施）、`review-07-stage07-docs-registry.md`（S07 文档登记，主 agent 直接执行）。
> 方法：7 个 review 单元串行（S01-S06 subagent、S07 主 agent），以各 Stage commit diff + checklist 六段式条目 + verify/stage-0X.md 断言为真值源对照，三维度（保真度/实现质量/新引入问题）+ 触碰文件全量复查（存量问题混排不分归因）+ 定向实跑；人工验证点不在本次范围（见末节清单）。
> 合计 **30 条问题**（2+4+6+6+5+3+4）。问题编号为文件内独立序列，引用须带文件名消歧（如 review-04 R2-FE-01 与 review-05 R2-FE-01 为不同问题）。

## 问题总表

### Stage 01 工具链与门禁（review-01，2 条）

| 编号 | 关联项 | 问题 |
|---|---|---|
| R2-TE-01 | TE-01 | main() 查询失败日志标签「网络/解析」未随非 2xx 语义同步，与头注口径不一致 |
| R2-TE-02 | —（存量） | vitest.config.ts exclude 整段死配置（include 已自足），且自定义 exclude 覆盖默认 node_modules 防护 |

### Stage 02 预览链路（review-02，4 条）

| 编号 | 关联项 | 问题 |
|---|---|---|
| R2-SEC-01 | SEC-02 | host_page_carries_domain_csp 断言面缺 script-src/style-src 'unsafe-inline' 两指令锁 |
| R2-SEC-02 | SEC-02 | checklist 写死的 L4 双 spec 验证未完整执行（markdown.e2e.ts 缺，由 S06 全量间接覆盖，偏差未登记） |
| R2-SEC-03 | SEC-03 | verify/checklist「catch(() => 零命中」断言字面不成立（:182 render 路径空参 catch 为存量保留形态） |
| R2-SEC-04 | SEC-03 | PreviewFrame「下轮轮询自愈」注释失实（轮询等值早退不重发 sync，真实重试源为几何变化/主窗移动） |

### Stage 03 后端加固（review-03，6 条）

| 编号 | 关联项 | 问题 |
|---|---|---|
| R2-BE-01 | BE-01 | test-exemptions.md CP-011 行用例计数失真（join_with_timeout 4 例实为 3 例，与 :24 行冲突） |
| R2-BE-02 | BE-01 | notify 测试 shutdown() 换装后丢失 watcher 线程 panic 检测（join 错误被吞，assert 恒过） |
| R2-BE-03 | BE-02 | fs 游标解码失败路径（含新增 tag/NUL 校验分支）全量无用例 |
| R2-BE-04 | BE-02 | 实现与 checklist 写死步骤偏差：601 夹具改 1201（已留痕、测试更强，登记为偏差项） |
| R2-BE-05 | —（存量） | spawn.rs:2155 注释「BE-06」标签与本计划 BE-06（基准两轮制）同号不同义，无就近解码 |
| R2-BE-06 | —（存量） | FileWatcher::drop 复制 stop() 逻辑而非委托（hooks/watcher.rs 已委托，notify 未收敛） |

### Stage 04 前端清理（review-04，6 条）

| 编号 | 关联项 | 问题 |
|---|---|---|
| R2-FE-01 | FE-01 | 「无快照首帧落地后恰好一次上呼」规格未被测试锁住（仅 toHaveBeenCalled ≥1） |
| R2-FE-02 | FE-01 | 生产环境无快照路径 onViewStateChange 双提交（微任务先于心智渲染，test env 结构性不可暴露） |
| R2-FE-03 | FE-01 | restoringRef 抑制态无超时兜底——loadRoot 永不 settle 则提交永久抑制 |
| R2-FE-04 | FE-01 | rootPath effect 注释解除点列举遗漏第 4 个解除点（loadRoot 首帧失败 catch） |
| R2-FE-05 | FE-02（存量） | stages.md/verify 写 FE-02 路径为 src/panels/panelRegistry.ts（实际 src/panelRegistry.ts），门禁断言不可原样执行 |
| R2-FE-06 | FE-10（连带） | knip.json 追加 `docs/compromises-fix-review-fix/**` 整目录 ignore 宽于必要（应收窄至 workflows/*.js） |

### Stage 05 大文件链路（review-05，5 条）

| 编号 | 关联项 | 问题 |
|---|---|---|
| R2-BE-01 | —（流程） | Stage 进度表登记错位两级（S04 由 S05 捎带、S05 由 S07 回补） |
| R2-FE-01 | FE-05 | 在途旧代际单飞结果可被复位后的新扫描直接消费——陈旧文本进新行索引且此后不再触发失效 |
| R2-FE-02 | FE-08（存量） | 外部修改重载路径（applyExternalChange）全量 readFile 无任何大小防线，与四层防线表述不自洽 |
| R2-FE-03 | FE-05 | 首挂 stat 基线时序竞态——base null 期间事件被跳过 + 基线后设于修改之后时陈旧索引失去再触发器 |
| R2-FE-04 | FE-05 | 「同 size 同 mtime 改写」假阴性窗口未登记（与 editor 域内容判等先例差异未说明） |

### Stage 06 e2e 设施（review-06，3 条）

| 编号 | 关联项 | 问题 |
|---|---|---|
| R2-TE-01 | TE-09 | WARN 计数按 chunk 独立匹配，跨 chunk 断行漏计（且多字节 UTF-8 截断乱码） |
| R2-TE-02 | TE-09（存量沿留） | fallback 通道 `process.exit(code)` 未做 `?? 1`，信号杀死被掩为 exit 0（与 runWdio 通道不一致） |
| R2-TE-03 | —（存量） | workspace/CLAUDE.md:75 右键菜单工厂签名过期（第 4 参已改回调形态，S11 连带修复未同步文档） |

### Stage 07 文档登记（review-07，4 条）

| 编号 | 关联项 | 问题 |
|---|---|---|
| R2-DOC-01 | SEC-02（收尾 5b4e12f） | 「全仓修正 11 处」失实——现役注释同义失实表述漏改 5 处（markdown-assets.test.ts:19、html.e2e.ts:163/293、markdown.e2e.ts:14/234） |
| R2-DOC-02 | SEC-02（收尾 5b4e12f） | adr.md:446/459/505 与 test-exemptions.md:39 历史区现在时「预览域无 CSP」表述无复核注记指引（仅 adr.md:460 有） |
| R2-DOC-03 | —（存量） | execution-plan.md 进度表计数自相矛盾（row 04「8 处」/列举实为 10 点位/row 07「11 处」） |
| R2-DOC-04 | DOC-02 | verify/stage-07.md 断言「.temp/node22 预置通道仍在」与常态不符（按需预置目录，常态不存在，断言恒假） |

## 门禁重跑结果（2026-09-11，主 agent 串行实跑）

| 门禁 | 结果 |
|---|---|
| `npx tsc --noEmit` | 绿 |
| `npx eslint src/` | 绿 |
| `cargo clippy -- -D warnings` | 绿（ts-rs 一条 parse note，非 warning） |
| `cargo fmt -- --check` | 绿 |
| L2 `npm test` | 绿（199 文件 / 3261 例） |
| L1 `cargo test -- --test-threads=1` | 绿（lib_tests 832 例 + 集成各 target 132 例，全过；scan_bench 两轮制未触发重采样） |
| `npx knip --production` | 绿（exit 0） |

## 附记（review 过程观察，不进问题清单）

- 任务指派的 S01/S02/S03 commit hash（37b7b50/47909e1/4a6457f）在仓内不存在，实际为 7abc009/d1a6838/89988d6——历史在 review 期间被改写（S04-S07 hash 未变），subagent 按 commit message 主题对应核验，范围无误。
- S06 首次 spawn 因子代理 API 额度中断，重试完成；S07 因主会话工具调用故障改由主 agent 直接执行（纯文档 Stage，串行纪律不受影响）。

## 待人工验证清单（本轮未做，修复后统一实机验证）

计划内（execution-plan.md 收尾节）：
- S01：CI 实跑绿（Guard — src/types step 首跑确认）；
- S02：预览回归全场景（html/markdown 缩放/滚动/导航）；
- S04：页签 chrome 行为冒烟（开页/关页/右键菜单/重命名）+ 布局/面板全场景（跨页拖拽/重启恢复）；
- S05：>10MB 文件实机滚动观感 + 外部修改失效重扫抽查；
- S06：TE-04 env 哨兵负向验证（改真实屋 env → exit 报红 → 还原）；TE-09 WARN 计数基线填记确认；
- 既有联动项：Node 26 全量 e2e（.bak 已删前置满足）、CP-029 定责落档复核、Win10 conpty 回退 toast、claude TUI 滚轮实测（ADR-0007 门禁第 3 条）、剪贴板空闲复跑。

本轮 review 新增需实机/实跑确认项：
- review-04 R2-FE-02：无快照路径双提交为生产时序推断（L2 act 环境结构性不可暴露）——实机确认注册表槽行为，或按建议收口后回归；
- review-05 R2-FE-01/R2-FE-03：blockCache 在途旧代际消费窗口、首挂基线竞态——实机抽查「外部修改浏览中的大文件」时顺带观测（修复后）；
- review-02 R2-SEC-02 连带：TE-05 探针降级后多 `--spec` 形态实跑（双 spec 第 2 位 worker 降级 warn 继续）——checklist TE-05 验证节要求，本轮静态 review 未覆盖。
