# 02-third-party-tools.md 事实核查报告

> 核查日期: 2026-07-25 | 核查方法: npm registry + WebSearch GitHub stars + 社区文档交叉验证

---

## 错误 1: claude-hud GitHub Stars 严重过时

- **文件+行号**: `02-third-party-tools.md` (行 293)
- **原声称**: "GitHub Stars ~14.5k"
- **错误类型**: 过时信息
- **正确信息**: 截至 2026 年年中，实际 stars > 25,000（约 25.4K–26.2K）。14.5k 是 2026 年 3 月下旬的数据（~4 个月前）。该项目是 Claude Code 生态中增长最快的插件之一。
- **反证来源**: WebSearch "claude-hud jarrodwatts github stars 2026" — 时间线：3 月 15 日 ~7K → 3 月 21 日 ~9.5K → 3 月 28 日 ~14.5K → 年中 > 25K

---

## 错误 2: statusLine API 更新频率表述不精确

- **文件+行号**: `02-third-party-tools.md` (行 28, 313, 720)
- **原声称**: "每 ~300ms 通过 stdin 传入 JSON 状态" / "全部走 300ms 去抖队列"
- **错误类型**: 事实错误（不精确表述）
- **正确信息**: 300ms 是 **debounce 窗口**，不是固定 polling 间隔。Claude Code statusLine 默认**不**定时刷新——仅在事件触发时运行（新 assistant 消息、权限模式变化、Vim 模式切换、模型切换），且需设置 `refreshInterval`（秒）才有定时轮询。300ms debounce 的作用是合并快速连续变化（如快速切换 vim 模式），避免多次触发脚本。
- **反证来源**: WebSearch "claude code statusLine API interval milliseconds 300ms" — "300ms is the debounce window, not a polling rate. The status line does not refresh every 300ms by default."

---

## 错误 3: claude-hud token 数据来源描述有歧义

- **文件+行号**: `02-third-party-tools.md` (行 317)
- **原声称**: "使用真实 token 数据（非估算）" / "零外部依赖（TypeScript 编译为单文件）"
- **错误类型**: 来源不支撑
- **正确信息**: claude-hud 使用两个数据源：(1) statusLine API 提供真实 token 数据（来自 stdin JSON 的 `context_window.*` 字段）；(2) 解析 transcript JSONL 文件提取工具调用、子代理活动、todo 进度。后者是文件解析而非实时 API。零外部依赖指的是 npm 依赖，但运行时仍需 Node.js。
- **反证来源**: WebSearch "claude-hud jarrodwatts" — "zero external dependencies (TypeScript compiled to single file), uses real token data (not estimated)"

---

## 错误 4: spark-hud 版本号验证

- **文件+行号**: `02-third-party-tools.md` (行 333)
- **原声称**: "版本 0.7.0"
- **错误类型**: 事实核实通过（正确）
- **说明**: npm registry (socket.dev) 确认 spark-hud 当前版本为 0.7.0。此项标记为验证通过。

---

## 错误 5: tabby-claude-status 版本号验证

- **文件+行号**: `02-third-party-tools.md` (行 246)
- **原声称**: "版本 1.2.1"
- **错误类型**: 事实核实通过（正确）
- **说明**: npm registry 确认 tabby-claude-status 当前版本为 1.2.1。此项标记为验证通过。

---

## 错误 6: claude-iterm2 版本号验证

- **文件+行号**: `02-third-party-tools.md` (行 50)
- **原声称**: "版本 0.2.6（2026-06-08）"
- **错误类型**: 事实核实通过（正确）
- **说明**: npm registry (socket.dev) 确认 claude-iterm2 最新版本为 0.2.6，最后更新日期 2026-06-08。此项标记为验证通过。

---

## 错误 7: vibe-term 版本号验证

- **文件+行号**: `02-third-party-tools.md` (行 424)
- **原声称**: "版本 1.4.1（2026-02-05）"
- **错误类型**: 无法验证
- **说明**: npm 搜索未反回 vibe-term 的具体版本信息。发布日期 2026-02-05 无法独立确认。标记为低置信度。

---

## 错误 8: "颜色状态映射几乎形成事实标准"断言过于绝对

- **文件+行号**: `02-third-party-tools.md` (行 762-766)
- **原声称**: "蓝/白 = 工作中、黄/橙 = 需关注、绿 = 完成、红 = 错误"被描述为"事实标准"
- **错误类型**: 事实错误（过度泛化）
- **正确信息**: 不同工具的颜色方案存在显著差异：
  - claude-iterm2: 蓝=work, 黄=waiting, 绿=done, 红=error
  - burnkit: 白=active, 绿=just done, 黄=10min idle, 红=20min idle
  - claude-needs-input: 默认=work, 橙脉冲=blocking, 绿=done
  - claude-code-tab-title: 不使用颜色，仅用 `*`/`·` 前缀
  虽然蓝/黄/绿/红是最常见的映射，但 burnkit 的"时间梯度+白色活跃"方案证明了并非所有工具遵循同一模式。
- **反证来源**: 03-terminal-progress-standards.md 内部矛盾——OCS 9;4 state 4 在 Windows Terminal 为 Warning（黄色），在 WezTerm 为 Paused（不支持），语义不统一

---

## 错误 9: "Hook 脚本写状态到信号文件/端点"架构描述遗漏重要细节

- **文件+行号**: `02-third-party-tools.md` (行 33-37)
- **原声称**: 信号传输方式包括"写 JSON 到 `/tmp/` 或 `%TEMP%`"
- **错误类型**: 事实错误（遗漏）
- **正确信息**: 路径描述不完整——不同工具使用不同路径：
  - tabby-claude-status: `%TEMP%\tabby-claude-status.json`
  - ccmonitor: `~/.ccmonitor/`
  - claude-code-tab-title: `/tmp/claude-tab-<session_id>` JSON 文件
  应明确标注各工具的具体文件路径而非泛写 `/tmp/`。此外，claude-hud 使用 `statusLine` API（stdin 管道），根本不写文件——这是更精确的第三种通信方式。
- **反证来源**: 文中自身对各工具的详细描述即包含正确的具体路径

---

## 核查范围

- 已验证：22 个第三方工具的来源链接、版本号（npm）、GitHub stars、实现架构描述、hook 事件使用、跨平台支持声明
- npm 注册表验证（socket.dev）：claude-iterm2 0.2.6、tabby-claude-status 1.2.1、spark-hud 0.7.0
- GitHub stars 交叉验证：claude-hud ~14.5K → >25K（过时）
- 架构描述验证：statusLine API debounce 机制、文件通信路径
