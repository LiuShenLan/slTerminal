# 妥协修复计划 Review 汇总

> 分文件：`review-01-deps.md`（依赖）、`review-02-backend.md`（后端）、`review-03-security.md`（安全）、`review-04-frontend.md`（前端）、`review-05-testing-cleanup.md`（测试+清理）。
> 方法：5 个 subagent 串行，以 git 改动面（c6758e5..6af3791）+ checklist/销项登记为真值源对照，三维度（保真度/实现质量/新妥协）+ 定向测试抽查；人工验证点不在本次范围（见末节清单）。
> 合计 **31 条问题记录，去重后 30 条**（preview label 字符集冲突经 review-02 与 review-03 双 agent 独立复核确认，为同一问题）。

## 问题总表

### 依赖与技术选型（review-01，7 条）

| # | CP | 问题 | 详 |
|---|---|---|---|
| 1 | CP-001 | check-ts7-trigger.mjs 不校验 HTTP 状态码，限流 403 合法 JSON 误落退出码 1 而非登记的 2 | review-01 #1 |
| 2 | CP-001 | check-ts7-trigger.d.mts 入参声明窄于运行期契约（缺 null） | review-01 #2 |
| 3 | CP-002 | 销项括注「五导出，JsonMode 生产接线」失实（实际七导出、仅二生产接线） | review-01 #3 |
| 4 | CP-003 | `.temp/node22.bak` 删除条件已满足（S12 全量 e2e 已跑）但残留未清，checklist 步骤 8 未执行 | review-01 #4 |
| 5 | CP-033 | verify/stage-10.md 声称红测演练「在 commit body 留痕」，全链 commit 实查不存在 | review-01 #5 |
| 6 | CP-033 | 「字体真实加载渲染」实证无可复核锚点，现存 L4 仅断言 data: 串存在、不验字体加载 | review-01 #6 |
| 7 | CP-033 | checklist/verify 的 B2 字面断言（局部 CSP 放行 font-src data:）与实际落地（预览域无 CSP）不符，执行文档未回写 | review-01 #7 |

### 后端架构与平台（review-02，9 条）

| # | CP | 问题 | 详 |
|---|---|---|---|
| 8 | CP-004 | **页前缀 panelId 含 ":" 与 preview validate_label 字符集冲突，生产预览链路四命令全被 Err(Validation) 静默拒绝**（e2e 裸 id 掩盖，review-03 #1 独立复核确认） | review-02 #1 |
| 9 | CP-004 | 页内分屏能力随共享宿主消亡，销项注记未披露该功能性回退 | review-02 #2 |
| 10 | CP-011 | 销项「无裸 join 无界阻塞路径」与 checklist 验证命令均与实仓不符（notify/hooks 三处生产裸 join 仍命中） | review-02 #3 |
| 11 | CP-006 | fs_read_dir 续页失败时 catch 无条件清空已渲染首帧，「续页失败可重试」注释语义不成立 | review-02 #4 |
| 12 | CP-006 | 偏移游标遇目录中途变长可跨页重复/遗漏，契约文档未登记该边界，前端聚合无去重 | review-02 #5 |
| 13 | CP-005 | grep 守卫自匹配：守卫描述行自身含 `std::sync::(Mutex|RwLock)` 字面量，按字面执行恒非零命中 | review-02 #6 |
| 14 | CP-004 | WorkspaceDockHost onReady 的 disposables 清理闭包无任何消费路径（卸载后注册表/`__dockviewApi` 残留旧引用） | review-02 #7 |
| 15 | CP-008 | git/mod.rs:185 注释仍列 `ignored`，死分支清理的注释残留 | review-02 #8 |
| 16 | CP-004 | PageDockviewHost.tsx 名实不符（宿主组件已消亡，仅存共享件，文件名未随） | review-02 #9 |

### 安全放宽（review-03，3 条）

| # | CP | 问题 | 详 |
|---|---|---|---|
| 17 | CP-012 | 同 #8（双 agent 交叉确认，此处补充：PreviewFrame 三处调用 catch 静默吞错，生产预览无声消失） | review-03 #1 |
| 18 | CP-012 | 预览域零 CSP 引入「预览内容任意出网」新通道——比 checklist 计划的局部 CSP 形态更宽，未登记为新妥协 | review-03 #2 |
| 19 | CP-013 | exportContextBindings/ExportedBinding 生产零消费残留 + shortcuts/CLAUDE.md:62 描述过时，退役面未清干净 | review-03 #3 |

### 前端架构（review-04，6 条）

| # | CP | 问题 | 详 |
|---|---|---|---|
| 20 | CP-016 | useFileTree 挂载即向注册表空提交，loadRoot 完成前卸载再切回则展开态丢失 | review-04 #1 |
| 21 | CP-017 | panelRegistry `"settings"` 裸字面量两处，与同文件命名常量形态不一致 | review-04 #2 |
| 22 | CP-022 | LargeFileViewer 行文本色模块级 import 期快照，与 CP-039 响应式方向相悖 | review-04 #3 |
| 23 | CP-022 | largeFile.sizeBytes 用 UTF-16 长度近似字节数，信息条大小对非 ASCII 文件系统性偏小 | review-04 #4 |
| 24 | CP-022 | blockCache 无文件变更失效，浏览期间外部修改产生新旧块混合视图且无提示 | review-04 #5 |
| 25 | CP-031 | 注入产物「不得含 `</script>`」拼接纪律无测试锁，失守静默 | review-04 #6 |

### 测试覆盖缺口 + 遗留清理（review-05，6 条）

| # | CP | 问题 | 详 |
|---|---|---|---|
| 26 | CP-024 | 销项「漂移守卫入门禁」失实——ci.yml 无 src/types Guard step，守卫仅为文档操作指令 | review-05 #1 |
| 27 | CP-046 | 哨兵键集合缺 `env`（套件自身 writeFakePlanEnv 写该键），存在漏检口；「唯一可能写入」声明失实 | review-05 #2 |
| 28 | CP-030 | e2e-tests/CLAUDE.md:116 同条自相矛盾（「正常速度」已被同条修正段否证未清理）；性能回归感知 sole 依赖宽松 timeout | review-05 #3 |
| 29 | CP-030 | TQ-E-10 探针副作用：多 `--spec` 定向调用确定性 fast-fail，runner 形态收窄未入销项注记 | review-05 #4 |
| 30 | CP-040 | 根 CLAUDE.md 红线括注「721 例等价覆盖」计数失真（现 825 例） | review-05 #5 |
| 31 | CP-046 | test-exemptions.md:70 仍写旧 sha256 整文件比对口径，文档同步遗漏 | review-05 #6 |

## 门禁重跑结果（2026-09-08，主 agent 实跑）

| 门禁 | 结果 |
|---|---|
| `npx tsc --noEmit` | 绿 |
| `npx eslint src/` | 绿 |
| L2 `npm test` | 绿（199 文件 / 3236 例） |
| L1 `cargo test` 全量 | 824/825；唯一红 = CP-007 基准用例（并行负载污染，见附记） |
| `cargo clippy -D warnings` | 绿（ts-rs 一条 parse note，非 warning） |
| `cargo fmt --check` | 绿 |

## 附记（review 过程发现，不进问题清单）

- **CP-007 基准门禁负载敏感**：L1 全量与 L2 并行跑时 `scan_bench_1000_sessions_median_under_200ms` 实测中位 410ms 报红，隔离复跑即绿。S06 已做防 flaky 加固但仍不抗并行负载——全量回归须串行执行，或基准改钳位/重试语义，否则 CI/本地并行场景会误报。

## 界外观察汇总（非本次修复引入，供另行处置）

- `vitest.config.ts:7` exclude 含损坏目录名 `'datalearncodeterax-ai-temp'`（去斜杠拼接产物，从未生效），建议清理（review-01）。
- `notify/mod.rs:968`、`hooks/watcher.rs:448` 测试代码内裸 join（生产零影响，备查）（review-02）。
- `useCodeMirror` 的 10MB 检查发生在全量读盘之后，「内存保护」不覆盖读盘一步（存量形态）（review-04）。
- `writeFakePlanEnv` 在 background-tasks/settings 两 spec 逐字重复，可收进 helpers.ts（与问题 #27 同源）（review-05）。
- review-05 补充核实：CP-023 的 89.55% 登记算术自洽；`#[coverage(off)]` 全仓零标注（无趁机豁免生产代码）；CP-040 embed-manifest「bins-only + 无宏 API」失实判定经 crate 源码一手证实，回退结论成立。

## 待人工验证清单（本次 review 未覆盖，修复问题后统一验证）

| 来源 | 验证点 |
|---|---|
| S03 / CP-003 | Node 26 全量 e2e 实跑（`.temp/node22.bak` 删除前置，与问题 #4 联动） |
| S03 / CP-029 | 定责结论落档复核（产品缺陷出口的磁盘核对补偿实机行为） |
| S03 / CP-046 | 负向验证：改真实屋 sentinel 键，断言 exit 校验报红（注意先修问题 #27 的 env 哨兵缺口） |
| S04 / CP-010 | Win10 实机：捆绑 conpty 回退时启动 toast 提示降级后果 |
| S06 / CP-007 | 扫描成本实测数字合理性确认（中位 180.33ms → 指纹分支决策） |
| S08 / CP-009 | claude TUI 滚轮实测：ConPTY 输入模式各开关形态（ADR-0007 门禁第 3 条） |
| S09 / CP-022 | >10MB 文件实机滚动/渲染观感抽查 |
| S10 | 预览回归全场景（html/markdown 缩放/滚动/导航）+ spike go 结论人工确认——**注意先修问题 #8/#17（生产预览链路当前被 validate_label 全拒），否则本项必红** |
| S11 | 布局/面板全场景（开页/关页/跨页拖拽/重启恢复/存量布局迁移）+ 剪贴板空闲复跑（终验 3 failed 为剪贴板外部占用环境豁免） |
