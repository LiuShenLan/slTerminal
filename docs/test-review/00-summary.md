# 测试用例质量 Review 汇总报告

> 审查日期：2026-08-22/23。方法：7 个审查域串行执行，实证（实跑基线/重跑/coverage 工具）+ 静态对照实现深查。
> 分报告：01 L1 Rust / 02 L2 工作区面板 / 03 L2 features / 04 L2 数据层基建 / 05 L3+L4 E2E / 06 CI+基建+inventory / 07 覆盖率缺口。

## 基线事实

| 层级 | 实跑结果 |
|------|----------|
| L1 Rust | 726 用例全绿（`--test-threads=1`）；llvm-cov 行覆盖 87.70% |
| L2 前端 | 154 文件 / 2635 用例全绿；v8 行覆盖 93.93%、分支 86.00% |
| L3 headless | 7 文件 / 138 用例全绿 |
| L4 E2E | 9 spec 全绿（两轮：2:45 / 2:27） |
| 静态门禁 | tsc / eslint / clippy / fmt 本地全过 |

## 问题统计（共 65 个：高 15 / 中 26 / 低 24）

| 维度 | 高 | 中 | 低 | 合计 |
|------|----|----|----|------|
| 覆盖率 | 6 | 8 | 8 | 22 |
| 断言有效性 | 3 | 3 | 4 | 10 |
| 稳定性与确定性 | 2 | 4 | 4 | 10 |
| 隔离性 | 1 | 3 | 3 | 7 |
| mock 合理性 | 1 | 3 | 0 | 4 |
| 基建 | 0 | 2 | 2 | 4 |
| inventory | 2 | 1 | 0 | 3 |
| CI | 0 | 2 | 1 | 3 |
| 并发 | 0 | 0 | 2 | 2 |

| 报告 | 高 | 中 | 低 | 合计 |
|------|----|----|----|------|
| 01 L1 Rust | 0 | 1 | 6 | 7 |
| 02 L2 工作区/面板 | 1 | 4 | 3 | 8 |
| 03 L2 features | 3 | 8 | 8 | 19 |
| 04 L2 数据层/基建 | 0 | 2 | 2 | 4 |
| 05 L3+L4 E2E | 5 | 3 | 2 | 10 |
| 06 CI+基建+inventory | 2 | 4 | 1 | 7 |
| 07 覆盖率缺口 | 4 | 4 | 2 | 10 |

## 共性根因聚类

1. **「替身/复制」脱节模式（最普遍）**：测试不复用生产实现，自行复制或手写替身——L3 复制 OSC 处理逻辑（05）、Watermark 手写组件（02）、file-viewer-registry 测试私有恢复函数（03）、barrel mock 只导出部分成员（02）、wire-keybindings 只测 fake store（03）。共同后果：生产漂移时测试仍绿（假阴性）。
2. **异步等待不当**：只等 DOM 不等组件初始化（02 实证 1/8 偶发失败）、`waitFor` 后紧跟同步断言（03 多处）、advanceTimersByTime(0) 假设微任务完成（03）、E2E 固定 350ms sleep（05）。
3. **全局状态污染/隔离缺口**：模块级全局 stub 不恢复（02）、side-effect import 混入注册表（03）、Zustand store 未统一重置（03）、`window.__dockviewApi` 残留（03）、setup.ts 全局 mock 隐性耦合全部 L2（06）、E2E beforeSuite 只重置 projects（05）。
4. **jsdom 无布局环境下断言可信度低**：虚拟化计数受 StrictMode 双渲染影响（03）、getBoundingClientRect 全局 mock 过宽（03）、CM6 几何测量 stderr 异常被吞（02）。
5. **inventory 真值源失实**：L1（724 vs 实跑 726）、L2（2633 vs 实跑 2635，段小计与行级差 43）双重失实（01/06）。
6. **覆盖缺口集中在启动链与 OS 交互路径**：main.rs 零覆盖、lib.rs setup 副作用、PTY Win32/ConPTY 核心、hooks signal/watcher 分支（07）。
7. **CI 门禁缺口**：无 rustfmt 门禁、无 timeout、无 npm 缓存（06）。

## 高严重度问题清单（15 个）

| # | 问题 | 出处 |
|---|------|------|
| 1 | `diff-panel.test.tsx` 脏态弹窗用例组合运行 1/8 偶发失败（未等 CM6 初始化即 dispatch）——**实证复现** | [02](02-l2-workspace-panels.md) |
| 2 | explorer-virtualization 用 jsdom 模拟视口 + 文本计数，StrictMode 双渲染下虚拟化失效也可能通过 | [03](03-l2-features.md) |
| 3 | sideBar/sideBarArea/activityBar 共享 sideViewRegistry 单例，side-effect import 可能混入真实视图破坏隔离 | [03](03-l2-features.md) |
| 4 | explorer-sandbox-race 名为竞态测试实为顺序执行，无法回归 DBG-10 时序故障 | [03](03-l2-features.md) |
| 5 | L3 production-osc.test.ts 复制生产 OSC 实现，生产变更不会令测试变红 | [05](05-l3-l4-e2e.md) |
| 6 | L3 keyboard.test.ts 只测 xterm 透传，生产快捷键链路（Ctrl+Shift+C/V、Shift+Tab）L3 无守卫 | [05](05-l3-l4-e2e.md) |
| 7 | L4 三处树展开固定 350ms sleep，flaky 温床且拖慢执行 | [05](05-l3-l4-e2e.md) |
| 8 | L4 history.e2e.ts 吞掉 createProject 失败，后续断言可能基于不存在的状态 | [05](05-l3-l4-e2e.md) |
| 9 | L4 terminal.e2e.ts 粘贴断言只查注入标记，未验证真实按键路径 | [05](05-l3-l4-e2e.md) |
| 10 | inventory L1 用例数/文件数双重失实（724 vs 726） | [06](06-ci-infra-inventory.md) |
| 11 | inventory L2 用例数失实且段小计与行级合计差 43 | [06](06-ci-infra-inventory.md) |
| 12 | `main.rs` 入口与 panic hook 零覆盖，启动链异常无法自动回归 | [07](07-coverage-gaps.md) |
| 13 | `lib.rs` setup 副作用（watcher 启动、statusline reinject、窗口事件）低覆盖 | [07](07-coverage-gaps.md) |
| 14 | PTY Win32/ConPTY 核心路径（spawn 67.95% / conpty_api 63.19% 行覆盖）覆盖不足 | [07](07-coverage-gaps.md) |
| 15 | agent-event 信号消费链路（signal.rs / watcher.rs）分支覆盖不全 | [07](07-coverage-gaps.md) |

## 各层一句话结论

- **L1 Rust**：基线 726 全绿、无高severity问题，主要风险在并发锁用例的测试替身与生产锁不一致、条件 skip 用例 CI 空跑；inventory 计数失实。
- **L2 前端**：154 文件全绿但存在 1 个实证偶发失败用例；「jsdom 布局假设 + 全局单例污染 + 替身脱节」是三类主要隐患。
- **L3 headless**：复制生产实现与只测 xterm 透传两个问题使 L3 对生产 OSC/快捷键的回归防护名不副实。
- **L4 E2E**：两轮全绿但固定 sleep、吞错、注入式断言三处设计级脆弱性会在环境变化时暴露；`retries:1` 掩盖观察面。
- **CI**：缺 rustfmt 门禁、无超时保护、无 npm 缓存；本地门禁清单与 CI 不一致。
- **覆盖率**：整体数字不低（前端 93.9% 行 / Rust 87.7% 行），但缺口高度集中——启动链（main.rs/setup）与 OS 交互（PTY/ConPTY/hooks 信号）恰是风险最高的部分；豁免清单 11 条中 10 条成立，SEC-17 审计日志豁免理由不完全成立。
- **inventory**：作为「唯一真值源」三处口径失实，且 L2 段小计与行级对不上，需重建登记纪律。

## 分报告索引

1. [01 L1 Rust 测试 Review](01-l1-rust.md)（7 个）
2. [02 L2 域A 工作区/面板核心 Review](02-l2-workspace-panels.md)（8 个）
3. [03 L2 域B features 域 Review](03-l2-features.md)（19 个）
4. [04 L2 域C 数据层/基建域 Review](04-l2-data-infra.md)（4 个）
5. [05 L3+L4 E2E Review](05-l3-l4-e2e.md)（10 个，附 [L4 基线日志](e2e-baseline.log)/[重跑日志](e2e-rerun.log)）
6. [06 CI+基建+inventory Review](06-ci-infra-inventory.md)（7 个）
7. [07 覆盖率缺口分析](07-coverage-gaps.md)（10 个 + 缺口总表 + 豁免清单核查表）
