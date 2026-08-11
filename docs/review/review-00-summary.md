# review-00 汇总

> 对象：multi-cli profile 重构（8 Stage，至 commit b704845）开发结果。
> 方法：验收核对（我直接复核 8 Stage verify 断言）+ 7 维度独立审查（只读 subagent 串行）+ 基线门禁实跑。
> 原则：只写问题。各维度详情见同目录 review-01 ~ review-08。

## 基线门禁（2026-08-10 实跑）

| 门禁 | 结果 |
|------|------|
| `npx tsc --noEmit` | ✅ |
| `npx eslint src/` | ✅ |
| `cargo clippy -- -D warnings` | ✅ |
| `cargo fmt -- --check` | ✅ |
| L1 `cargo test --test-threads=1` | ◻ 未复跑——`slterminal.exe`（PID 4108，debug 产物）运行中致链接器文件占用（os error 5），不杀用户进程；采信收尾报告 b294c67：592 用例全绿 |
| L2 `npm test` | ✅ 2408 用例 / 139 文件全绿（与收尾报告一致） |
| L3 `npm run test:l3` | ✅ 138 用例 / 7 文件全绿（与收尾报告一致） |
| L4 `npm run e2e` | ◻ 未复跑，采信收尾报告 b294c67：39 用例（9 spec，37 active + 2 skip）全绿 |

## 问题总览

| 报告 | 维度 | P0 | P1 | P2 | P3 | 计 |
|------|------|----|----|----|----|----|
| review-01 | 验收核对 | 0 | 0 | 1 | 4 | 5 |
| review-02 | 正确性 | 0 | 1 | 4 | 2 | 7 |
| review-03 | 架构约束 | 0 | 0 | 0 | 0 | 0 |
| review-04 | 安全 | 0 | 1 | 3 | 0 | 4 |
| review-05 | 契约一致性 | 0 | 0 | 0 | 0 | 0 |
| review-06 | 测试质量 | 0 | 2 | 1 | 0 | 3 |
| review-07 | 文档一致性 | 0 | 0 | 0 | 4 | 4 |
| review-08 | 扩展性达成 | 1 | 3 | 3 | 0 | 7 |
| **合计** | | **1** | **6** | **11** | **12** | **30** |

## 重点问题（P0 + P1）

| 编号 | 级别 | 问题 | 位置 |
|------|------|------|------|
| KZ-1 | P0 | hub 编辑器槽无条件渲染 ClaudeHooksConfigEditor，无按 cliId 分派——新 CLI 声明 hasConfigEditor 即踩坑 | `HooksConfigPanel.tsx:32/:254` |
| ZQ-3 | P1 | hook 事件通道建行写死 `newStatus ?? "attention"`——null 映射事件（普通 Notification/未识别事件）首达时建幽灵 attention 行 | `useAgentStatus.ts:194` |
| AQ-1 | P1 | buildResumeCommand 单引号包裹 cwd 未转义——路径含单引号解析失败；篡改 JSONL cwd 可构造命令注入（需文件系统写权限为前提） | `strategies.ts:109-112` |
| CS-1 | P1 | AC-5 守卫词法器跳过含 `${}` 的模板字符串——`cl${''}aude` 拼接可绕过字面量禁令 | `no-claude-literals.test.ts` extractStringLiterals 模板分支 |
| CS-2 | P1 | AC-5 守卫七路径未含 `src/features/cliProfiles` 根目录（registry/types 属通用层） | `no-claude-literals.test.ts:39-47` |
| KZ-2 | P1 | AgentEventPayload.transcriptPath 必填且语义绑死 claude transcript 概念——CLI 不中立 | `types/agent.ts:27` / `signal.rs:31` |
| KZ-3 | P1 | CliHooksProvider trait `context_usage(transcript_path)` 参数名把 claude 概念写进跨 CLI 签名 | `hooks/provider.rs:33` |
| KZ-4 | P1 | HooksLayer 硬编码 user/project/local 三层——配置分层模型不同的 CLI 无法表达 | `types/hooksConfig.ts:8` |

**校注**（reviewer 汇总时核实补充）：
- KZ-1：MC-223 决策 2 已声明「claude 专属编辑器不抽象」为本期范围决策；本条指出的实质是**接口契约（hasConfigEditor 布尔）与实现（无条件 claude 编辑器）之间的陷阱**——属扩展点缺失，非本期功能缺陷。级别在扩展性维度语义下成立。
- AQ-1：代码注释（strategies.ts:106-107）已自述「单引号未转义，原实现遗留限制，原样保留」——非本次重构引入，安全维度将其升级为注入面标注。
- CS-2：cliProfiles 根目录当前生产代码无 "claude" 字面量（本次 grep 复核零命中）——属守卫覆盖空洞，非现存违规。
- KZ-5（P2）与 KZ-1 同源：claude 路径字面量位于已声明的 claude 专属编辑器内，修复编辑器分派后自然消解。

## 跨维度共性问题

### 族 1：边界输入防御缺失（正确性 × 安全）

复合键与三级解析对「非预期值」一律无防御：消费方无 `?? CLAUDE_CLI_ID` 回退（ZQ-1，HistorySessionList.tsx:278 与三处拼接方不一致）、空串 cliId 穿透 `??` 回退（ZQ-2，三处同构）、分隔符 `|` 无转义（ZQ-7）。同族：信号文件无大小限制（AQ-2）、删除链跟随符号链接（AQ-3）。**统一修复方向**：抽 `keyOf(cliId, sessionId)` 单点（回退+转义一处生效），后端信号/删除路径补防御性校验。

### 族 2：文档/注释失实（验收核对 × 文档一致性 × 扩展性）

11 条，占总数 1/3——本次重构的最大问题族。根因：8 Stage 并行改动后文档同步存在盲区。三类形态：
- **退役 API 引用残留**：YS-1（P2，panels/CLAUDE.md:249 与同文件 :275 自相矛盾，最重）、YS-2、YS-3（7 处 TabTitleRegistry 模式先例）、YS-5
- **描述与代码漂移**：YS-4（claudeHistory 注释）、WD-2（JSON 模式补全已删但三处文档仍声称有）、WD-4（注释写单 sessionId，实为复合键）
- **登记遗漏**：WD-1（根索引漏登 F9）、WD-3（CONTEXT.md 后端注册表描述失实）、KZ-6（「新增 CLI 步骤」漏后端 provider 注册/test-inventory/编辑器分派三步）

### 族 3：claude 概念泄漏进通用契约（扩展性 × 测试质量）

transcriptPath 必填（KZ-2）、trait 参数名 transcript_path（KZ-3）、HooksLayer 三层锁死（KZ-4）、hub 编辑器无分派（KZ-1）、守卫不扫 cliProfiles 根（CS-2）、mock 编辑器掩盖分派缺失（KZ-7）。**本期仅 claude 一个 CLI，全部无实际故障；但「易扩展」目标部分未达成**——新增第二个 CLI 时，前后端契约层需一轮中性化改造（transcriptPath 可选化、trait 参数更名、layer 抽象、编辑器注册表）。

### 族 4：E2E 隔离的异常路径（安全）

AQ-4：fixture 缺失时 `SLTERM_CLAUDE_PROJECTS_DIR` 不设置，后端回落真实 `~/.claude/projects`——E2E 有读（并潜在删除）用户真实历史会话的路径。修复方向：fixture 缺失即终止，不降级。

## 决策与人工确认状态（2026-08-11 更新）

1. **人工验证点**：✅ 全部通过（2026-08-11 真机实测，completion-report.md 验证点清单闭环；含决策 7「版本过旧 → 重新注入」升级路径确认可用）。
2. **修复决策**：**全部 30 条问题列入本期修复**（2026-08-11 拍板）——含 KZ 族 claude 概念中性化（KZ-1/2/3/4）「本期偿还」，形态决策：hub 编辑器分派 = 组件入 profile；usageSourcePath 更名 = trait + 命令参数全链路；配置层抽象 = profile 声明层集合。
3. **L4 未复跑**：采信收尾报告 b294c67（39 用例全绿）；修复完成后随本轮改动统一复跑。
4. **L1 本次未复跑**：原因与采信依据见「基线门禁」表；修复完成后关闭运行中的 slterminal.exe 统一复跑。

## 报告索引

| 文件 | 内容 |
|------|------|
| review-01-验收核对.md | 8 Stage 断言逐条复核矩阵 + 5 条问题（YS-1~5） |
| review-02-正确性.md | 7 条（ZQ-1~7） |
| review-03-架构约束.md | 0 条（11 条硬约束 + 新增纪律全绿） |
| review-04-安全.md | 4 条（AQ-1~4）+ 六检查点通过项 |
| review-05-契约一致性.md | 0 条（四层契约对账全绿） |
| review-06-测试质量.md | 3 条（CS-1~3） |
| review-07-文档一致性.md | 4 条（WD-1~4）+ 11 检查点通过项 |
| review-08-扩展性达成.md | 7 条（KZ-1~7）+ 11 触点通过项 |
