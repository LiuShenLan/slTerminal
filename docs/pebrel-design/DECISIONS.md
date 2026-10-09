# 设计澄清裁决单（DECISIONS)

> 2026-10-07 与用户逐轮澄清(grill 6 轮)后的裁决单源。各设计篇「待沉淀决策」/「开放问题」节为过程稿,**与本单冲突时以本单为准**;实现期(M 阶段)以本单落地,符合 ADR 三条件者届时沉淀 `.claude/adr.md`。

## A. 逐条裁决(17 条)

| # | 决策 | 裁决 | 备注 |
|---|---|---|---|
| D2(01) | 数据目录叶子名 | **`slterm`**(`%APPDATA%\slterm`) | 用户推翻推荐;全标识符一致 |
| D3(01) | 数据文件命名 | **去品牌**(`settings.json`/`providers.json`/`history.jsonl`) | 目录名已带品牌;魔数联动见 D10-1 |
| D06-1 | settings.json 键形态 | **段嵌套**(appearance/scrolling/… 各成段) | 用户推翻推荐;写通道按段浅合并 |
| D12-4 | 首发版本线 | **续 0.3.0**;更新链对 <0.3.0 一律「需手动重装」 | Tauri 安装基不构成 Inno 可更新对象 |
| D04-1 | CLI 子系统取向 | **console 子系统**;Runtime 控制面保留(AI CLI agent 是其主要消费者) | GUI 启动闪控制台窗接受;标准流正确性优先 |
| D05-1 | WindowState 回放 | **回放**(记住上次窗口大小/位置) | schema 字段语义 = 行为输入 |
| D05-2 | 侧栏骨架 | **双槽**(ActivityBar + 上下槽,R1-R9 纯函数随迁) | |
| D05-3 | 侧栏状态归属 | **session**(collapsed/width/active_view 跟现场恢复) | |
| D07-2 | 通用 diff 形态 | **双栏对照** | |
| D07-3 | 编辑器字体键域 | **共享单键**;**不做旧场景配置迁移**(用户明示,泛化信号) | |
| D07-1 | git 执行体 | **子进程统一**(与 pebrel 同构;git2 语义资产转为测试保留) | spec 分片 07 事实修正后的重裁 |
| D02-1 | DA1 应答身份 | **`?64;22c`**(slTerminal 现役值;M2.4 真机复测钉死) | priming/Term 自答/reader 代答三处同身份 |
| D11-1 | lib_tests | **每 crate 显式 test target**(`[lib] test=false` + `[[test]]`) | TQ-COV-06 防线机制化 |
| D11-2 | 测试入口 | **`cargo xtask` 封装**(`cargo xtest <filter>`) | |
| D06-4/D09-1 | blur/opacity | **五材质枚举(默认 None)+ opacity 标量**,06 字段一次到位锚定 | 无 bool 占位 |
| D06-3 | 内置主题 | **pebrel 暗色子集 + linear 作默认主题入目录** | |
| D10-1 | 备份魔数 | **`TRMBK001`**(去品牌格式标识,8 字节 AAD) | 与 D3 同理由 |
| D10-3 | plan_balance 推送 | **模块级 watch channel**(多消费者,不耦合组件树) | |
| D10-4 | 备份 UI 入口 | **06 设置页安全分区**(恢复点清单同面) | |
| D08-2 | hook 失败边 | **默认关**(与 AI assistant 默认关闭一致) | |
| D06-2 | 旧 settings.json | **零迁移**:新版只读写 `%APPDATA%\slterm`,旧 exe 同级文件不看不迁;仅 projects.json 按已裁定提取 | |
| — | GitHub 仓 | **已存在**:`SLTERM_RELEASES_OWNER=LiuShenLan`、`SLTERM_RELEASES_REPO=slTerminal` | 真值落定,09/12 篇占位回填 |
| —(review 后) | WSLENV | **砍**:spec 03 照抄合并为误裁,WSL 全家不采纳为准;03 篇删 WSLENV 合并臂及测试点 | 2026-10-09 review 裁决 |
| —(review 后) | console 阻塞父 shell | **登记+明示**:D04-1 备注;12 篇安装器/文档明示快捷方式与 Start-Process;启动链改造子步归 13 篇 | 同上 |
| —(review 后) | windows_console_startup 对照 | **保留重设**:对照目标重设为 console 形态启动行为,子步归 11 篇 | 同上 |
| —(review 后) | 首发 GitHub Release | **首发即开**:首发即挂 Release,自动更新链公网验收 | 同上 |
| D13-1 | Ctrl+Shift+C 无选区语义 | **静默吞**(照抄 pebrel;不透传中断) | 批量默认同类 |

## B. 批量确认默认(25 条,用户全部接受)

1. D1 slterm_config → 并入 settings(无第二实现的抽象税)
2. D4 禁名门禁 → 新增行 + M1 全量清零 + 清零后全树常挂(docs/ 临时稿豁免至 M11)
3. D02-2 ConPTY 发行 → 两阶段合并(单文件 exe 优先,提取成本实测登记)
4. D02-3 scrollback → 10000 随 pebrel(M3 实测后走设置键再评)
5. D03-1 AgentSession.source → 宽松落盘、严格消费(AgentKind::parse 归一,不归一不可恢复)
6. D03-2 SEC-12 词表 → 初版 M4.3 定,10 审计域登记 + note 链演化
7. D03-3 statusline 介质 → 信号文件保留(app 数据目录,与 hook 管道分通道)
8. D03-4 流键 → 砍 remote_process(键 = source+session+pane+agent_pid+bridge_instance)
9. D04-2 契约数值 → 首版照搬 pebrel,实测校准登记;发布后即成事实契约
10. D04-3 windowing_behavior → 删键,单窗行为硬编码归 09
11. D08-1 AssistantConfig → 独立文件照 pebrel
12. D08-3 gpui-component 补丁 → 最小集(数学渲染必需),补丁清单与回采纪律登记
13. D09-2 更新首发型态 → setup.exe(pebrel 同构)
14. D10-2 备份类别清单 → M10 实现期按 06 键域终态重定
15. D11-3 内存阈值 → M10 实测一周稳定基线后锁入检查点
16. D12-1 Inno AppId GUID → 新立 v4 钉死,夹具另立,两值永不变更
17. D12-2 体积预算 → 全量功能态实测 + 15% 裕度,注释登记实测值与校准日期
18. D12-3 永久资产 → 根级 `assets/`(skills/schema/协议文档)+ 根级 `release-notes/`
19. 01开放2 竞品名禁表 → 不迁(禁名门禁只管 nebula/pebrel 回流)
20. 01开放4 projects.json → 提取,层级保留,数据归 05 消费、07 只读引用
21. 08开放1 Posix 方言 → 保留(Git Bash/MSYS 场景)
22. 11开放1 conformance → 协议桩为主;真 CLI 登记实机验收点(默认 shell + 一家 CLI 冷启动)
23. 11开放2 pr-size → 建 `pr_size_hint.ps1` 不挂 check_all(按月/release 提示)
24. 11开放3 实机验收 → M11 前全量过 + M 阶段出口前可选,写进豁免表执行方式列
25. 12开放3 arm64 → 立短 note 登记(Revisit when = arm64 设备需求或 GPUI 上游支持变化)

## C. 已闭环开放问题(无需再议)

- 04开放1 Schema 归置 → D12-3(assets/)
- 04开放2 conformance 桩 → B.22
- 04开放4 移交窗口还原 → 09 已答并回登 04 缝合点 5(单窗:聚焦唯一窗 + tab.new)
- 08开放4 spec 分片 08 命名偏差 → 从 01 表唯一权威,M11 统一性 pass 回改 spec
- 09开放1 05 标题栏文字冲突 → 按 spec 归 05,05 篇文字待修(review 项)
- 09开放3 Releases 坐标 → A 表 GitHub 仓真值
- 10开放3 plan_balance 旧数据 → D06-2 零迁移(不迁)
- 05开放3 projects 层级 → B.20
- 06开放2 theme_library 文件名 → 随 D3 去品牌
- 01开放5 Inno GUID → B.16

## D. 实现期延后项(默认已明,随 M 阶段落地,不再 grill)

02开放1 参数转义择优(M2.3 用例对比删一)/ 02开放2 Registry 锚点(M3 校准)/ 02开放3 DSR 观察项 / 02开放4 inline image 默认归壳 / 02开放5 真机门禁=M2.4 能力验证+12 发布复核 / 03开放1 codex 子代理(M4.2 实测)/ 03开放2 parse_command 归一(M4.5)/ 03开放3 agent_history 旧数据(可接受损失)/ 03开放4 bridge_instance 特判位置(M4.2)/ 03开放5 hooks 页换代(归 06)/ 05开放1 veil 主题开关归 06 / 05开放2 tab 菜单归 M8 / 05开放4 pane 标题条展示归 07 / 05开放5 导出入口 04/07 各自接线 / 06开放1 i18n payload 两语实测 / 06开放3 搜索别名消费测试(M6.2)/ 06开放4 hooksConfigGui 判定函数(M6.5)/ 06开放5 主题同名消歧(M6.3)/ 07开放1 扩展名对账(实现期)/ 07开放2 gutter 值集收敛(随 D07-1)/ 07开放3 选区菜单(M8.5)/ 07开放4 图标载体(M8.4)/ 07开放5 预览高亮(M8.3)/ 08开放2 数学语料归置(11/12)/ 08开放3 PSReadLine 并存(联评,suppressed_line 复用)/ 10开放1 审计落盘(实现期)/ 10开放2 恢复点 UI(随 D10-4 设置页)/ 01开放1 python 探测序(M0.3 实测)/ 01开放3 GPUI 跟随(锁同 rev 集)/ 12开放2 Tabby 段(02 篇 M2 后回填)
