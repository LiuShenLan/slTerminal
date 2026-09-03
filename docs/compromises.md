# 妥协问题清单

依据 `.claude/CLAUDE.md`「开发取向(未来最优)」原则对存量登记的盘点(2026-09-02)。

**收录判据**:以「成本不抵收益 / 动现状风险大 / 接受次优」为由关闭的成本型妥协与工程债务——未来最优原则下可重新评估修复。

**排除项与去向**:
- 自动化测试不可守卫的纪律型豁免 → 原档 `.claude/test-exemptions.md`
- 产品定位 / 用户已否决偏好 → 根 CLAUDE.md「项目性质」、各 ADR
- 外部依赖节奏约束(xterm beta、notify RC——上游无稳定版可升,升级触发条件已登记)→ ADR-0007 / ADR-0008
- 平台/浏览器能力边界(cmd.exe 无 shell integration、Tauri 通知无点击路由等)→ 所属模块 CLAUDE.md

**用法**:逐条修复,完成后勾选销项。条目格式:来源(登记点)→ 当时理由 → 问题本质。修复若触及登记点原文(ADR / 模块 CLAUDE.md),须同步更新,勿留失真登记。

---

## 一、依赖与技术选型

- [ ] **CP-001 · 双 TS 并存**(TS6 包装器 + `@typescript/native` TS7 别名)
  来源:ADR-0010 TE-07。当时理由:typescript-eslint 8.67.0 peer 依赖全系拒绝 TS7(加载期硬校验),三支 fallback 走尽后正式化妥协。问题本质:类型检查(TS7)与 ESLint 消费(TS6)各走一套编译器,依赖版本矩阵双轨;消除依赖上游 issue #10940 闭环 + TS7.1 发布。
- [ ] **CP-002 · json-schema-library 9.x / 11.x 双 major 并存**
  来源:ADR-0010 TE-15、`src/features/cliProfiles/CLAUDE.md`。当时理由:codemirror-json-schema@0.8.1 锁 9.x(上游约束),主声明 11.6.2;运行时两实例无冲突。问题本质:同一库双实例并行,体积与语义双份,待上游升级消解——评估是否有替代方案不再经 codemirror-json-schema 引 9.x。
- [ ] **CP-003 · E2E 工具链版本妥协(便携 Node 22 启动器)**
  来源:`e2e-tests/CLAUDE.md`。当时理由:Node≥26 自带 undici 8 与 webdriverio 不兼容,自动下载便携 Node 22 兜底。问题本质:测试运行时依赖自动下载第三方二进制,版本矩阵游离于主 toolchain;评估升级 webdriverio 或固定受控 Node 版本纳入 npm scripts。

## 二、后端架构与平台

- [ ] **CP-004 · 多 Dockview 实例架构 + `MAX_PAGES=20` 上限缓解**
  来源:ADR-0009 FE-01(含 FE-36 跨项目全局计数修订)、`src/workspace/CLAUDE.md`、`src/stores/CLAUDE.md`。当时理由:H6 终端跨页面存活 + xterm 实例约束,架构上每页一 Dockview 实例,以页数上限防内存/DOM 线性增长。问题本质:缓解阀而非根治——实例数仍随页面线性增长,容器/渲染管线重复;未来最优应为共享宿主或虚拟化面板池。
- [ ] **CP-005 · 后端 `std::sync::Mutex` 中毒保持现状**
  来源:ADR-0009 09#14、`src-tauri/src/CLAUDE.md`、`src-tauri/src/pty/CLAUDE.md`、`src-tauri/src/git/CLAUDE.md`(仓库缓存)。当时理由:临界区短小无 panic,中毒不可达,parking_lot 换装零收益。问题本质:以「当前无 panic」论证原语次优性,未消除「未来锁内引入 panic → 等待方连锁 panic」的架构脆弱点;换原语或 catch_unwind 仅作预案而未决。
- [ ] **CP-006 · `fs_read_dir` 返回整目录列表不分页**
  来源:ADR-0009 BE-21、`src-tauri/src/fs/CLAUDE.md`(红线「禁止加分页」)。当时理由:改分页 = IPC 契约破坏性变更,收益不抵成本;懒加载 + FileTree 虚拟化(FE-30)覆盖渲染侧。问题本质:IPC 契约形态被现状成本冻结——大目录单次返回整表的内存峰值与首帧延迟仍在,契约应按理想终态(分页/游标)重设计并同步迁移前端。
- [ ] **CP-007 · session 扫描缓存目录级粗粒度失效键**
  来源:`src-tauri/src/agent_history/CLAUDE.md`(BE-19,红线「缓存键语义勿改」)。当时理由:失效键为 `(mtime, file_count)` 目录级粒度,目录内增删不失效,由前端 `force` 兜底。问题本质:缓存正确性依赖上层强制刷新,弱一致窗口客观存在;应评估文件级指纹或目录内容哈希的失效精度。
- [ ] **CP-008 · `git_status` 切除被忽略文件扫描 + `is_ignored()` 死代码滞留**
  来源:`src-tauri/src/git/CLAUDE.md`(红线「不要恢复 include_ignored」)。当时理由:50K+ ignored 文件致数秒 I/O 阻塞,切除后 `is_ignored()` 分支保留为「无害死代码」防回潮。问题本质:以红线 + 死码双锁固化的性能取舍——功能语义(git_status 不看 ignored)与渲染需求是否真正对齐未验证,死代码增加静态分析噪音。
- [ ] **CP-009 · PASSTHROUGH_MODE(0x8)永久禁用**
  来源:`src-tauri/src/pty/CLAUDE.md`。当时理由:0x8 致 claude 全屏 TUI 滚轮失效,为兼容牺牲该输入模式能力。问题本质:一个输入模式为单一兼容场景整体废弃,claude 上游行为演进后无自动恢复路径;模式能力矩阵应可配置/可回归验证而非二值禁用。
- [ ] **CP-010 · Win10 捆绑 conpty 提取/加载失败静默回退系统 conhost(0x3)**
  来源:ADR-0005、`src-tauri/src/pty/CLAUDE.md`。当时理由:部署形态(exe+dll 两文件)红线优先,回退不阻断使用;失败仅 `tracing::warn!`。问题本质:降级不透明——用户无感知落入无鼠标滚轮转发(0x3)的老 conhost 路径;失败应具可观测性(UI 警示或状态暴露),而非静默劣化。
- [ ] **CP-011 · `pty_kill` 3s 超时放弃 join 仅 warn**
  来源:`src-tauri/src/pty/CLAUDE.md`。当时理由:ClosePseudoConsole 在 pre-24H2 可永久阻塞,容忍超时降级。问题本质:进程销毁路径存在无界阻塞风险以超时妥协——平台缺陷修正前,超时后的残留句柄/进程清理策略缺失。

## 三、安全放宽

- [ ] **CP-012 · CSP `script-src 'unsafe-inline'` + `dangerousDisableAssetCspModification` 放宽**
  来源:ADR-0009 SEC-09、`src/panels/CLAUDE.md`(红线勿收紧)。当时理由:srcdoc iframe 继承父 CSP(W3C 行为),HTML 预览注入脚本必须内联,移除即破坏预览。问题本质:主应用失去 script nonce 加固,`default-src 'self'` 为唯一远程脚本防线——预览通道与主应用共享同一 CSP 宽松度,隔离(独立 webview / 沙箱化 iframe 资源域)未做。
- [ ] **CP-013 · HTML 预览 nonce 可被注入脚本内部伪造(威胁模型登记)**
  来源:ADR-0010 D16/SEC-04、`src/panels/CLAUDE.md`。当时理由:不防预览 HTML 内联脚本内部伪造,接受,以 global 命令集最小化兜底。问题本质:nonce 机制对「同源注入脚本」威胁的防护形同虚设——真正的边界应是把预览内容与宿主 global 上下文隔离(命令面收窄到最小暴露)。
- [ ] **CP-014 · shell 路径比对双侧 canonicalize 失败回退归一字符串(SEC-15 残余风险)**
  来源:ADR-0010 D15/SEC-15、`src-tauri/src/pty/CLAUDE.md`。当时理由:alias/Store 版 pwsh 兼容保留;收窄为「两侧均失败且归一化字符串完全相同」才放行。问题本质:残余的字符串级放行路径仍是伪安全比对(不解析真实文件身份),alias 场景的合法用例与绕过手段在该路径下不可区分。
- [ ] **CP-015 · hooks 卸载时非法 JSON 静默跳过配置清理仍删目录**
  来源:`src-tauri/src/hooks/CLAUDE.md`。当时理由:部分清理降级——配置解析失败不阻断目录删除。问题本质:破坏性操作在数据未确认清理时照常执行,错误静默;非法配置应显式上报后再决定是否继续删目录。

## 四、前端架构

- [ ] **CP-016 · 侧栏视图换区重建,组件内部状态丢失**
  来源:ADR-0001(已确认接受)、`src/features/sideViews/CLAUDE.md`、`src/features/explorer/CLAUDE.md`(展开状态)等多登记点。当时理由:换区低频(设定一次后不改),重建成本低于跨父节点保持实例的架构复杂度。问题本质:状态(展开树/rootNodes/滚动)与挂载父节点耦合,跨区即丢——状态应上移(由视图注册表或 store 持有)而非绑死在组件实例上。
- [ ] **CP-017 · settings 面板不入 `isAlwaysRenderPanel`,dirty 随卸载丢失**
  来源:`src/workspace/CLAUDE.md`(SC-FE-06,决策写死)。当时理由:与旧 hooksConfig 行为一致继承,不新增 always 渲染内存开销。问题本质:脏表单状态随面板卸载静默丢失(未保存修改),「行为一致继承」延续了旧缺陷而非修正;dirty 状态应持久于壳层而非组件。
- [ ] **CP-018 · WebGL 检测不带 `failIfMajorPerformanceCaveat`**
  来源:`src/panels/CLAUDE.md`(FE-26)。当时理由:blocklist 场景会连同软件渲染拒绝 → DOM renderer 快滚掉帧;SwiftShader 远快于 DOM 全帧重建,接受软件渲染。问题本质:GPU blocklist 机器落入慢速软件渲染且无回退提示——渲染路径选择对硬件能力检测粗糙,未给用户任何降级信号。
- [ ] **CP-019 · PTY spawn 布局等待 30 帧/500ms 超时回退 80×24**
  来源:`src/panels/CLAUDE.md`。当时理由:极端场景降级尺寸。问题本质:超时后以默认尺寸建立终端,若真实布局稍后到达产生 resize 抖动;等待与回退策略是计时猜测而非事件驱动(布局就绪即建)。
- [ ] **CP-020 · Ctrl+C 中断滞留 `working` 状态已知行为(登记不修)**
  来源:`src/panels/CLAUDE.md`。当时理由:CC 中断不发射 hook 事件,状态机无中断出边,滞留至下一事件/60s idle_prompt 转 attention。问题本质:用户按下中断后 UI 长时间保持「working」假象——状态机缺「中断」事件源,以上游事件缺失为由接受误导性 UI。
- [ ] **CP-021 · 历史区相对时间无 ticker,不自动刷新**
  来源:`src/features/agentHistory/CLAUDE.md`(MC-318,「视为可接受,不修」)。当时理由:渲染时计算,等其它状态变更触发重渲染。问题本质:页面静止时相对时间戳(「5 分钟前」)随时间腐化失真,以「等其它变更」为托辞——最低成本是订阅级 tick 刷新渲染。
- [ ] **CP-022 · CodeMirror 大文件不虚拟化,10MB 硬上限**
  来源:ADR-0009 FE-31、`src/panels/editor/CLAUDE.md`。当时理由:CM6 文档模型不支持部分加载;分块 + 10MB 上限 + 1MB 警告三层防线削峰。问题本质:编辑器能力以 10MB 为界被框架冻结,超限文件直接拒开——若大文件场景必要,需虚拟化/只读分片浏览路径而非依赖上游。

## 五、测试覆盖缺口

- [ ] **CP-023 · Rust 行覆盖 88.20%,距 90% 目标差 1.8pp 收尾登记**
  来源:`.claude/test-exemptions.md` TQ-COV 收尾。当时理由:残余缺口集中 PTY Win32 分支 + main.rs 结构性零覆盖 + 编译器生成物计数缺失。问题本质:覆盖目标以登记收尾而非达成——缺口处正是平台耦合最深的代码,可测化重构(抽取纯逻辑/依赖注入)可系统性收敛。
- [ ] **CP-024 · IPC 后端必填缺失 → invoke reject 被调用方 catch 吞 = 契约绿但运行时静默失败**
  来源:`src/ipc/CLAUDE.md`。当时理由:mockIPC 只守 JS 侧形状,真实序列化契约由 L4 兜底。问题本质:前后端 DTO 契约漂移在单元层不可见,失败信号被前端 catch 吞成静默——契约一致性(硬约束 #4)缺一层类型/运行时双端核对机制。
- [ ] **CP-028 · e2e 导航树展开辅助 children 计数循环对无会话页面行不收敛(奇偶翻转风险)**
  来源:`e2e-tests/history.e2e.ts` ensureProjectPagesExpanded(2026-09-03 已改单次点击版)、`e2e-tests/agent.e2e.ts` ensureTreeExpanded、`e2e-tests/mockcli.e2e.ts` 同构展开循环。当时理由:页面行无活跃会话时不渲染子级容器,DOM 无法区分展开/收起——以「每轮 children 计数判定,奇数次翻转必然到达展开稳态」假设收敛;2026-09-03 历史节点收进项目展开容器后,四态用例项目行被前置展开、6 轮循环全被页面行 toggle 消耗(偶数翻转终态收起)暴露缺陷,history spec 处已改「每行至多点击一次」,agent/mockcli 同构循环**暂时保留**(仅靠「每用例新建项目、项目行收起起点消耗首轮」巧合通过)。问题本质:DOM 无状态可判据时用计数循环赌奇偶,语义脆弱——统一方案 = 展开判定不依赖 DOM(测试侧记录点击态或组件暴露展开探针),合适时机与 agent/mockcli 循环一并修改。

## 六、遗留清理与同步点

- [ ] **CP-025 · 退役模块目录遗留(sidebar)**
  来源:`src/features/sidebar/CLAUDE.md`(NAV-06)。当时理由:「本目录待清理:目录删除时本文件一并删除」。问题本质:退役代码与文档滞留仓库,目录与根索引/引用形成误导(存在即被读)。
- [ ] **CP-026 · `useAgentStatus` 数据 hook 留存(视图已退役)**
  来源:`src/features/agentStatus/CLAUDE.md`。当时理由:数据层为导航树保留,组件层删除。问题本质:消费方仅剩导航树但 hook 仍驻退役模块目录——归属错位,应迁移至实际消费模块或并入导航树数据层后清理。
- [ ] **CP-027 · 启动链 fail-safe 三处静态色硬编码,手动同步**
  来源:ADR-0002、`src/theme/CLAUDE.md`(改 linear 值须同步 index.html/tauri.conf.json/main.tsx)。当时理由:静态层(index.html/tauri.conf.json)无法用 TS token,收编被否决,注释交叉引用兜底。问题本质:配色单点(硬约束 #6)在此三处破口,人工同步是腐化源——评估构建期注入(打包脚本改写 / 运行时早期读取)闭合缺口。
