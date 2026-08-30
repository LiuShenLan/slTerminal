# F11 设置中心执行结果 Review 汇总

> 范围：commit `2154493..825ed56`（F11 全 9 commit）diff + 全量波及面。
> 方法：3 个 review subagent 串行执行（根因定位 / 测试覆盖质量 / 文档一致性+偏离审计），分报告各自落盘，本文件去重汇总。
> 只写问题（每条附修复建议）；三份分报告维度正交，无跨报告重复条目。

## 一、根因定位结论（置顶）

**现象**：打开项目 → 关闭客户端 → 重启 → 侧栏可见、主区空白、导航树项目列表为空。

**物证**：`src-tauri/target/debug/slterminal-projects.json` 完整且为合法 JSON（项目数据未从磁盘消失；`.bak` 亦完整）。

**根因（高）**：启动加载路径的静默失败链——
1. `src/stores/projects.ts:233-256` `loadFromDisk` 内部 try/catch 吞掉所有异常，失败后仅 `console.warn`，store 保持 `{}`；
2. `src/App.tsx:85-91` `loadAllProjects()` catch 同样静默，且**无条件**执行 `markPersistenceReady()`（:91）打开持久化门控——此后任何 store 变更都会把空状态 2s debounce 回写磁盘（数据自杀链，`src/stores/projects.ts:295-297,316-322`）；
3. 空 store → 导航树空（useNavTree.ts:97 直订阅 `s.projects`）、主区空白（无 allPages）。

**F11 的关联**：F11 未改动加载/保存实现（diff 实证），不是根因；但设置面板进入布局 JSON 提高了保存频率，间接暴露既有静默失败问题。最可能触发点为重启瞬间 `load_projects` IPC/文件读取失败（文件锁/句柄竞争），静态分析无法 100% 锁定运行时根因——分报告给出 4 个候选根因排序与各自的区分验证方法（候选 2「exe 目录漂移导致读错数据目录」与用户部署习惯相关，值得优先实测排查）。

**修复建议**（详见 review-01 第 2 节）：
1. `loadFromDisk` catch 不静默、错误上抛；`App.tsx` catch 后不调 `markPersistenceReady()`，向用户报错并提供「重试/空状态继续」选择；
2. `loadFromDisk` set 前加结构校验（`data.projects` 为对象）；
3. `saveToDisk`/`saveAllProjects` 加空 projects 拒写守卫（防自杀链）；
4. 后端 `load_projects` 加 `tracing::error!` 审计日志。

**附带发现**（review-01 第 3 节）：
- （高，与根因同链）`loadAllProjects` 失败/E2E 跳过两路径都无条件 `markPersistenceReady()`——「未加载」与「加载失败」均进入可写模式。
- （中）SC-FE-08 自动关闭 effect 对 store 水合时序敏感（`SettingsPanel.tsx:350-398`），建议加 loaded/ready 门控。
- （低）SettingsPageProps 未显式标注「mount 期禁止调用 onPageParamsChange」约定，未来新增页组件有误触发保存风险。

---

## 二、问题总表（去重后 15 条，按严重度排序）

| # | 级别 | 问题 | 位置 | 来源 |
|---|------|------|------|------|
| 1 | 高 | 启动加载静默失败链（根因：多层 catch 吞错 + 无条件 markPersistenceReady + 空状态可回写） | stores/projects.ts:233-256,295-322；App.tsx:85-91 | review-01 |
| 2 | 高 | 需求规格仍把 schema 单点标在已删除的 `src/features/hooksConfig/` | docs/settings-center-requirements.md:189 | review-03 |
| 3 | 中 | SC-FE-08 自动关闭 effect 对 store 水合时序敏感（缺 ready 门控） | src/panels/settings/SettingsPanel.tsx:350-398 | review-01 |
| 4 | 中 | E2E helper `__slterm_e2e_getSettingsPanelCount` 只计活跃页面（checklist 约定「全部页面」），未登记偏离 | e2e-tests/helpers.ts:325-334 | review-03 |
| 5 | 中 | 需求规格面板注册流程写了不存在的 `panels/index` barrel | docs/settings-center-requirements.md:170 | review-03 |
| 6 | 低 | 设置中心注册页 pages.ts 无 L2 直接守卫（测试 mock 掉真实注册） | src/__tests__/settings-panel.test.tsx:19 | review-02 |
| 7 | 低 | persistSelectedCli 迁移后丢失 3 条纯函数语义用例（43→37 例） | src/__tests__/settings-hooks-page.test.tsx:782-796 | review-02 |
| 8 | 低 | settings-panel pageParams 用例未断言 saveLayout 落盘（假绿空间） | src/__tests__/settings-panel.test.tsx:248-270 | review-02 |
| 9 | 低 | L4 未覆盖 × 关闭 dirty 守卫（仅 L2 覆盖，未登记豁免） | e2e-tests/settings.e2e.ts | review-02 |
| 10 | 低 | L4 用例⑧「同项目切页面板保留」断言过弱（未断言 panelId 归属） | e2e-tests/settings.e2e.ts:575-606 | review-02 |
| 11 | 低 | 迁移测试残留未使用的 mockApi/mockContainerApi 死代码 | settings-hooks-page.test.tsx:40-52；hooks-config-sync.test.tsx:60-67 | review-02 |
| 12 | 低 | L4 后端 settings.json 路径硬编码（cwd 依赖，备份集合不一致） | e2e-tests/settings.e2e.ts:64 | review-02 |
| 13 | 低 | SettingsPageProps 缺「mount 期禁止调用 onPageParamsChange」约定标注 | src/features/settingsCenter/types.ts | review-01 |
| 14 | 低 | 测试迁移文档新增文件清单遗漏 3 个文件 | src/__tests__/CLAUDE.md:69 | review-03 |
| 15 | 低 | DefaultTab filePath 判据注释残留已退役的 hooksConfig 字样 | src/workspace/PageDockviewHost.tsx:423 | review-03 |

级别分布：高 2 / 中 3 / 低 10。

**疑点（证据不足，需运行时验证，不计入上表）**：重启瞬间文件锁/句柄占用（需 Process Monitor 复现）；`load_projects` IPC 启动时序失败（需抓 WebView2 console + 后端 tracing 日志）。区分验证方法见 review-01 第 4 节。

## 三、修复建议索引

每条问题的具体修复建议在分报告对应条目内：

- 根因修复链（4 条，含自杀链守卫）→ [review-01-root-cause.md](review-01-root-cause.md) 第 2 节
- 测试补强（pages.ts 注册守卫新测试 / persistSelectedCli 语义补例 / saveLayout 断言 / × 关闭 L4 用例或豁免登记 / 用例⑧断言强化 / 死 mock 清理 / 路径推导）→ [review-02-test-quality.md](review-02-test-quality.md) 各条目
- 文档修正（规格两处失实改写 / 测试清单补 3 文件 / 注释 hooksConfig→settings / helper 计数二选一：改实现或补登记偏离）→ [review-03-docs-drift.md](review-03-docs-drift.md) 各条目

## 四、分报告索引

| 报告 | 维度 | 问题数 |
|------|------|--------|
| [review-01-root-cause.md](review-01-root-cause.md) | 根因定位专项 | 根因 1（高）+ 附带 3（高1/中1/低1）+ 疑点 2 |
| [review-02-test-quality.md](review-02-test-quality.md) | 测试覆盖质量（L1/L2/L3 计数实跑核对与清单一致） | 7（全低） |
| [review-03-docs-drift.md](review-03-docs-drift.md) | 文档一致性 + 偏离审计 | 5（高1/中2/低2） |
