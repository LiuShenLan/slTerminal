# 设置中心（F11）执行收尾报告

> 执行：`/systematic-changes-execute`（2026-08-30）｜计划产物：`docs/settings-center/`（execution-plan / stages / checklist / workflows / verify）

## 一、Stage commit 列表（main，7 个 Stage + 计划归档）

| # | commit | message |
|---|--------|---------|
| 计划归档 | `2154493` | docs(settings-center): F11 设置中心需求规格与分阶段执行计划产物归档（任务前既有） |
| 01 | `d23ceaa` | feat(plan_balance): 轮询间隔运行期可改 + plan_balance_set_interval 专用命令（F11） |
| 02 | `cfae69e` | feat(settings): 设置中心框架（注册表/壳/打开编排）+ 套餐余量频率页（F11） |
| 03 | `560c77c` | feat(settings): hooks 配置迁入设置中心，hooksConfig 面板类型退役（F11） |
| 04 | `72e812f` | feat(settings): 快捷键可视化配置页（F11） |
| 05 | `23b0336` | feat(settings): 切项目自动关闭设置中心面板（F11） |
| 06 | `3349e20` | test(e2e): 设置中心 L4 覆盖 + hooks 用例适配（F11） |
| 07 | `f9c7618` | docs(settings): F11 文档同步——术语/ADR-0012/模块文档/用例清单 |

每 Stage 顺序：Workflow 编排 → 全量测试门禁 → 逐项验证（verify 断言文件为唯一真值源）→ commit。

## 二、修复循环记录（2 次，均 1 轮闭环）

| Stage | 失败项 | 根因与修复 |
|-------|--------|-----------|
| 03 | SC-FE-05g / SC-FE-06c | ① diff-panel.test.tsx:482 一次 flaky（与本 Stage 零相关，单文件复跑 40/40 全绿确认）；② `grep openHooksConfig src/` 5 处注释残留（globalCommands.ts 失实注释改写 + restoreSession/openSettings 溯源注释同步） |
| 06 | SC-E2E-01b / SC-E2E-02d / L4 计数 | ① app.test.tsx:190 `(window as any)` 补 eslint-disable；② settings.e2e.ts 两处断言期望值 `Ctrl+W` → `Ctrl+KeyW`（keystroke 规范格式含 Key 前缀，显示层正确，纯断言笔误）；③ 修复后 e2e 全量 10/10 确认 L4 计数 50 |
| 07 | SC-DOC-01b | requirements.md §8 决策记录 16/17/18 行已补（R1/R2/R3，checklist 真值源达成）；ADR-0012 决策段补 R2 标签消内部不一致 |

## 三、最终测试状态（当前 HEAD = f9c7618）

| 层级 | 结果 | 命令 |
|------|------|------|
| L1 | 815 用例全绿 | `cargo test --test-threads=1`（Stage 07 门禁） |
| L2 | 166 文件 / 2839 用例全绿 | `npm test`（Stage 07 门禁） |
| L3 | 8 文件 / 142 用例全绿 | `npm run test:l3`（Stage 07 门禁） |
| L4 | 10 spec / 50 用例全绿 | `npm run e2e` 最终复核（日志 `%TEMP%\slterm-e2e-final.log`，10/10 100% completed，exit 0） |

静态门禁：tsc / eslint / clippy(-D warnings) / rustfmt 全绿（每 Stage + fix-loop 重复校验）。

## 四、人工验证点实测结果

计划标注 3 项人工验证点，核对结论如下（机器可验证部分已由 L2/L4 常驻用例覆盖，最终 e2e 全绿即为实测）：

| 人工验证点 | 覆盖方式 | 结论 |
|-----------|---------|------|
| 配置钮全链（打开 / hooks 三层编辑 / 注入卸载 / 切 CLI dirty / 旧布局丢弃 / × 关闭确认） | settings.e2e ①⑩ + hooks.e2e 既有全链 + layout-serde 9c（旧布局白名单过滤）+ workspace-defaulttab × 拦截 4 例 | ✅ 全部覆盖，e2e 全绿 |
| 录制屏蔽（Ctrl+Shift+C 不复制 / 改绑生效 / 保留键拒绝） | settings.e2e ⑥（合成 KeyboardEvent + 2s debounce 落盘断言）+ shortcuts.test suspended 2 例 + settings-keybindings 保留键红字拒绝 | ✅ 全部覆盖，e2e 全绿 |
| 频率页动态间隔（改 120 → 余量刷新节奏变化） | settings.e2e ④（真实后端落盘 `planBalance.intervalSec=120` + 余量刷新闭环断言）——「连续节奏肉眼观察」为纯人工项 | ✅ 落盘与立即生效已断言；连续节奏观察需用户真实应用肉眼确认（可选） |

## 五、未修复项

无。2 次 fix-loop 均 1 轮闭环，allFixed 全达成。

## 六、执行期实证记录（供后续任务参考）

1. **e2e 与 vite build 不得并行**：`npm run e2e`（build:e2e 的 cargo 编译期间）与独立 `npx vite build` 同写 `dist/`——后者覆写导致打进二进制的无 E2E helper 版，全部 spec 卡「Workspace 未就绪」（Stage 06 首轮实证，修复 = 串行）。
2. **diff-panel.test.tsx:482 既有 flaky**：外部 Modify 自动重载用例未等右栏 view 就绪即触发 fsEventCb，与设置中心零相关；已两次复跑全绿确认，未动该文件（surgical changes 纪律）。
3. **× 关闭守卫判据偏差（已登记）**：dockview-react 8.1.0 `IDockviewPanelProps` 无 `panel` 属性（计划假设 `panel.view.contentComponent` 只在 context menu 场景成立）——实现改用 `params.panelId` 的 `settings-` 前缀判据（与 dirtyRegistry 键同源），workspace/CLAUDE.md 已按此登记。
4. **keystroke 规范格式含 Key 前缀**：`Ctrl+Shift+KeyC` 形态（修饰键 + event.code），e2e 断言曾误写 `Ctrl+W`。
5. **`__slterm_e2e_getSettingsPanelCount` 计数范围收窄（已登记）**：checklist 约定「全部页面」，实现只计活跃页面 api——设置中心为全局单例，活跃页面计数即可等价覆盖功能断言，本修复计划已收口径（DOC-04）。
