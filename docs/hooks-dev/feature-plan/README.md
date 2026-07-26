# Claude Code Hooks 宿主侧增强 — 功能开发规划

> 本文档经 grilling 问答逐轮确认（2026-07-26），仅定义「开发哪些功能」及行为级边界条件，不含实现方案（技术选型、代码结构、开发排期均不在本文档范围）。
> 标注约定：**【已确认】** = 用户在问答中明确拍板；**【推导默认】** = 由已确认决策直接推出、未经单独确认的细化，开发前可复核。

## 文档地图

| 文件 | 内容 |
|------|------|
| `README.md`（本文） | 背景、调研依据、方向决策、功能索引、完整排除项、决策溯源 |
| [phase1-status-core.md](phase1-status-core.md) | 阶段 1：F1 信号文件通道 + F2 一键注入/卸载 + F3 页签四态指示 |
| [phase2-notify-overview.md](phase2-notify-overview.md) | 阶段 2：F4 任务栏闪烁 + toast 通知 + F5 Agent Status 侧栏视图 |
| [phase3-config-panel.md](phase3-config-panel.md) | 阶段 3：F6 hooks 双模式配置面板 |
| [ADR-0001-signal-file-channel.md](ADR-0001-signal-file-channel.md) | 架构决策：信号文件通道 + SLTERM_PANEL_ID 路由 |
| [ADR-0002-hook-disable-state-location.md](ADR-0002-hook-disable-state-location.md) | 架构决策：单条 hook 禁用状态存 slTerminal 侧 |

调研依据：`../hooks/`（D1–D5 五份报告，内容已经多轮核对，直接引用不复核）。术语表：仓库根 `CONTEXT.md`「Hooks 宿主侧增强」节。

## 背景与调研依据

| 报告 | 关键结论 | 对本规划的影响 |
|------|---------|---------------|
| D1 视觉反馈 | 30+ hook 事件、社区状态映射共识（工作/注意/完成/错误/空闲）；第三方工具标准架构 `hooks → 信号文件 → 终端适配器`；多 tab 管理是重度用户核心痛点 | F1/F3 的状态映射与通道选型；F5 的会话总览动机 |
| D2 配置管理 | hooks 配置三层嵌套 + 5 种 handler 字段各异；官方 JSON Schema 已存在（SchemaStore）；VS Code 双模式编辑是最佳参照；JetBrains Master-Detail 布局 | F6 的 UI 形态与功能构成 |
| D3 日志可视化 | **官方无 hook 运行时遥测**（Issue #50287，社区最大痛点）；exit code 0/1/2 语义按事件类型而异 | F1 通道必须自建的根因；A3 日志面板（已排除）的出处 |
| D4 PTY 集成 | hook 无 TTY（无法直接向终端写序列）；环境变量传递链 `终端→shell→claude→hook`；Windows 已知 bug（#48009 空 stdin、#23554 SessionStart 冻结）；PTY 层已就绪无需新功能 | SLTERM_PANEL_ID 路由的可行性基础；F1 脚本的风险清单 |
| D5 优秀项目 | stdin/stdout JSON 是行业标准；iTerm2 Triggers/VS Code 双模式是配置 UI 参照；通知生态 6+ 独立工具验证需求 | F4 的价值佐证；方向 B（已排除）的出处 |

## 方向决策

**仅做方向 A（CC hooks 宿主侧增强）【已确认 Q1】**

- 方向 A：slTerminal 不改动 Claude Code 本身，为其 hooks 提供状态可视化与配置管理外围能力
- 方向 B（不纳入）：slTerminal 自有终端级 hook 系统（OSC 133 生命周期 hooks、iTerm2 Triggers 式配置、Problem Matchers）——独立大系统，与"CC hooks 优化"主题不符，未来单独立项评估

D4 结论：PTY 层已就绪（ConPTY flags=0x7、COLORTERM/TERM/TERM_PROGRAM 注入正确），本轮无任何 PTY 层改动。

## 总体架构（逻辑视图）

```
Claude Code（hook 事件）
   │  stdin JSON → slTerminal hook 脚本（F2 注入）
   ▼
信号文件（JSON，约定路径，含 SLTERM_PANEL_ID + transcript_path）
   │  后端监听 → IPC 推送
   ▼
前端：页签四态（F3）/ toast+任务栏（F4）/ Agent Status 视图（F5）

transcript JSONL（~/.claude/projects/...）──被动解析──▶ 上下文用量条（F5）

配置面板（F6）◀──读写（校验+原子写）──▶ 三层 settings.json（user/project/local）
```

## 功能清单索引

### 阶段 1：状态可视化核心（最小闭环）

| 编号 | 功能 | 一句话 |
|------|------|--------|
| F1 | 信号文件通道 | hook 脚本把事件写 JSON 信号文件，SLTERM_PANEL_ID 精确路由到页签，后端监听推送前端 |
| F2 | 一键注入/卸载 | 手动按钮把状态上报 hooks merge 注入 user 层 settings.json，可干净卸载 |
| F3 | 页签四态指示 | 页签 emoji 图标反映 工作⚡/注意🟡/完成✅/错误❌，事件驱动覆盖，删除旧自定义图标功能 |

### 阶段 2：通知 + 会话总览

| 编号 | 功能 | 一句话 |
|------|------|--------|
| F4 | 任务栏闪烁 + toast 通知 | 窗口失焦时就权限请求/完成/错误发系统 toast，注意态任务栏闪烁，点击聚焦页签 |
| F5 | Agent Status 侧栏视图 | 当前项目所有 claude 会话列表（标题+四态+上下文用量条），点击跳转聚焦 |

### 阶段 3：配置管理

| 编号 | 功能 | 一句话 |
|------|------|--------|
| F6 | hooks 双模式配置面板 | Dockview 新面板：GUI 表单 / JSON 双模式编辑三层 settings.json，Schema 校验 + matcher 测试 + 单条启停 |

## 排除项（明确不做）

| 排除项 | 是什么 | 排除理由 | 何时应重议 |
|--------|--------|---------|-----------|
| 方向 B：自有终端级 hook 系统 | 基于 OSC 133 C/D 的 shell 生命周期 hooks + iTerm2 Triggers 式配置面板 + Problem Matchers | 独立大系统，服务对象是"所有 shell 命令"而非 CC hooks，超出本轮主题【已确认 Q1】 | CC hooks 增强落地后，若需对非 claude 命令做自动化再单独立项 |
| A3 hook 执行日志面板 | hook 执行历史表格（事件/exit code/耗时/stdout/stderr）+ 实时指示 + 过滤搜索 + 终端关联 | 用户明确排除（2026-07-26 问答）【已确认】 | 用户重新提出"调试 hook 难"诉求时 |
| A1-d OSC 9;4 进度序列 | 解析 `ESC]9;4` 驱动页签进度环/任务栏进度条 | 通用终端能力，与 CC hooks 主题关联弱【已确认】 | 做"通用终端能力增强"主题时自然纳入 |
| A2-c 社区模板库 | 一键插入社区验证的 hook 配置模板（危险命令拦截/自动格式化等） | 用户明确排除【已确认】。注意：F2 一键注入的内置配置片段**保留**——那是 slTerminal 自身运转所需，不是面向用户的模板库 | 配置面板上线后用户反馈"不知如何写 hook"时 |
| 跨层复制/移动 hook | 把某条 hook 从 user 层复制/移动到 project 层 | 锦上添花，三层分别编辑已覆盖绝大多数场景【已确认 Q12】 | 团队共享配置成为高频操作时 |
| 启动引导注入提示 | 首次启动 claude 页签时弹窗引导注入 hooks | 用户选择仅手动按钮，打扰最少【已确认 Q11】 | 若功能发现率过低（用户不知道有注入按钮） |
| .bak 备份 | 保存配置前备份原文件 | 校验不过拒绝保存 + 原子写已保证写不坏，编辑器有撤销【已确认 Q10】 | 出现配置写坏事故时 |
| 配置层级合并可视化 | 展示四层（managed/local/project/user）合并后的生效视图 | D2 标注高难度，收益不匹配【已确认】 | 用户对"为什么这条 hook 生效/不生效"困惑频发时 |
| D4 增强项 | CLAUDE_ENV_FILE 可视化、session-envs 缓存清理入口、环境变量传递链诊断 | D4 自身评估为低优先级【已确认】 | 对应痛点实际出现时 |
| PTY 层任何改动 | ConPTY flags、注入变量之外的 PTY 行为变更 | D4 结论：已就绪，且 PASSTHROUGH_MODE 有吞鼠标输入的前车之鉴 | 出现明确 PTY 层缺陷时（需实测真实 claude 验证） |

## 决策溯源

| # | 决策点 | 结论 | 理由 |
|---|--------|------|------|
| Q1 | 方向范围 | 仅方向 A | 贴合"CC hooks 优化"主题；B 是独立大系统 |
| Q2 | 感知通道 | 信号文件为主 + JSONL 为辅 | 唯一覆盖 hook 执行事件的通道 + 零配置补上下文用量。详见 ADR-0001 |
| Q3 | 会话→页签路由 | SLTERM_PANEL_ID 环境变量 | 精确路由，cwd 匹配在同项目多页签时有歧义。详见 ADR-0001 |
| Q4 | 页签呈现 | 仅 emoji 图标 | 零配色系统改动，与现有页签风格一致 |
| Q5 | 状态生命周期 | 事件驱动覆盖；启动即🟡；删旧自定义图标 | 第三方工具标准做法，最简单可靠 |
| Q5b | 删除范围 | 只删图标留标题；OSC 133 C 启动检测保留 | 标题切换仍有价值；OSC 133 C 提供未注入时的降级状态 |
| Q6 | 视图范围 | 当前项目 claude 会话列表 | 直击多 tab 管理痛点，范围收敛到当前项目避免噪声 |
| Q7 | 视图深度 | 仅列表行 | 用户裁减，工具活动流/代办进度不做 |
| Q8 | 通知规则 | 失焦时权限+完成+错误 | 三类均为通知生态验证的高价值场景，失焦门控防打扰 |
| Q9 | 配置 UI 形态 | 双模式一体 Dockview 面板 | VS Code settings 双模式典范；Master-Detail 需要大空间 |
| Q10 | 保存安全 | 校验拒绝 + 原子写 + 生效提示 | 改的是 claude 自己的配置，永远不能写坏 |
| Q11 | 注入形态 | 仅手动按钮，user 层 | 打扰最少；状态上报与项目无关 |
| Q12 | 多层级跨层 | 仅切换编辑 | 覆盖绝大多数场景，跨层操作为锦上添花 |
| Q13 | 禁用实现 | 禁用状态存 slTerminal 侧 | 官方无单条禁用字段，非标准字段污染用户文件。详见 ADR-0002 |
| Q14 | 分期 | 阶段1→2→3 | 依赖顺序：通道先行，通知/总览自然延伸，配置面板独立可用 |

## 后续建议（开发前）

- ~~为「信号文件通道 + SLTERM_PANEL_ID 路由」写 ADR~~ → 已落地：[ADR-0001](ADR-0001-signal-file-channel.md)、[ADR-0002](ADR-0002-hook-disable-state-location.md)
- ~~术语表落入 CONTEXT.md~~ → 已落地：仓库根 `CONTEXT.md`「Hooks 宿主侧增强」节
- 阶段 1 开发前：复核 phase1 文件中所有【推导默认】条目
