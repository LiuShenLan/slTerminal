# 第三方 Claude Code 视觉反馈工具调研

> 日期：2026-07-25
> 目的：调研利用 Claude Code hooks 提供视觉反馈的第三方终端工具，为 slTerminal 的 hooks 状态可视化设计提供参考。

---

## 目录

1. [集成方式总览](#集成方式总览)
2. [终端 Tab 视觉反馈](#终端-tab-视觉反馈)
3. [状态栏 / HUD](#状态栏--hud)
4. [多会话监控 / TUI 仪表盘](#多会话监控--tui-仪表盘)
5. [Agent 专用工具](#agent-专用工具)
6. [VS Code 集成](#vs-code-集成)
7. [其他值得关注的项目](#其他值得关注的项目)
8. [Claude Code statusLine API 架构](#claude-code-statusline-api-架构)
9. [分类对比矩阵](#分类对比矩阵)

---

## 集成方式总览

第三方工具与 Claude Code 的集成主要有三种技术路径：

| 路径 | 机制 | 延迟 | 典型工具 |
|------|------|------|---------|
| **statusLine API** | Claude Code 通过 stdin 传入 JSON 状态（事件驱动 + 300ms 去抖窗口，非固定轮询间隔），脚本输出到 stdout 渲染 | 实时 | claude-hud, claude-status-bar |
| **Hooks 事件** | 在 `~/.claude/settings.json` 注册 shell 脚本，Claude Code 在生命周期事件时执行 | 事件驱动 | claude-iterm2, burnkit, vibe-term, joy |
| **Hooks → 文件 → 轮询** | Hook 脚本写状态文件，独立守护进程轮询文件并更新视觉表面 | 1s 轮询 | tabby-claude-status, ccmonitor, claude-code-iterm2-tab-status |

**关键约束**：hook 脚本的 stdout/stderr 会被 Claude Code 捕获（不显示给用户），且 hook 进程无 controlling tty。因此工具必须通过以下方式之一触达终端：
- 写文件 → 外部进程轮询
- 走进程树找到 pty 设备 → 直接写 ANSI/OSC 序列
- iTerm2 Python API（仅 macOS）
- Windows COM API（仅 Windows Terminal / Tabby）

---

## 终端 Tab 视觉反馈

### 1. claude-iterm2

| 属性 | 内容 |
|------|------|
| **名称** | claude-iterm2 |
| **作者** | banyudu |
| **平台** | macOS + iTerm2 |
| **安装方式** | Claude Code 插件市场：`claude plugin install iterm2@claude-iterm2` |
| **版本** | 0.2.6（2026-06-08） |
| **来源** | https://socket.dev/npm/package/claude-iterm2 |

**可视化方式**：

| 状态 | Tab 颜色 | 行为 |
|------|---------|------|
| Working（处理中） | 蓝色 | 静态 |
| Waiting（等待输入） | 黄色 | 渐变动画（黄色→橙色，60s 周期） |
| Done（完成） | 绿色 | 短暂绿色闪烁后静置 |
| Error（工具失败） | 红色 | 短暂红色脉冲后静置 |

附加视觉表面：
- **Badge 文字**：Tab 角落显示当前状态和经过时间
- **桌面通知**：通过 `terminal-notifier` 推送
- **可配置音效**：每个状态可选音效
- **网格布局**：`/iterm2:grid 2x2` 创建多窗格布局
- **会话分叉**：`/iterm2:fork` 分叉到新窗格或 Tab

**使用的 Hook 事件**：`SessionStart`、`Stop`、`PreToolUse`、`PermissionRequest`

**实现架构**：
```
Claude Code hooks → 预打包 JS（插件内）→ iTerm2 逃逸序列 + iTerm2 API
```
- 预打包 JavaScript，零 npm install
- 通过 ANSI 逃逸序列设置 Tab 颜色
- Badge 通过 iTerm2 专有 OSC 序列
- 桌面通知调用 `terminal-notifier`（需 `brew install terminal-notifier`）

**配置**：`~/.config/claude-iterm2/config.json`，支持 `tabColor`、`badge`、`notification`、`doneToWaitingDelay`（自动过渡到 waiting 的秒数）、`gradientDuration` 等开关。

---

### 2. claude-code-iterm2-tab-status

| 属性 | 内容 |
|------|------|
| **名称** | claude-code-iterm2-tab-status |
| **作者** | JasperSui |
| **平台** | macOS + iTerm2 |
| **安装方式** | Claude Code 插件：`/plugin install iterm2-tab-status@jaspersui-marketplace` |
| **来源** | https://github.com/JasperSui/claude-code-iterm2-tab-status |

**可视化方式**：

| 状态 | 前缀 | Tab 行为 |
|------|------|---------|
| Running（工作中） | 闪电 | 静态 |
| Idle（空闲） | 休眠 | 静态 |
| Attention（需关注） | 红点 | **橙色闪烁** + Badge |

点击闪烁的 Tab 停止闪烁。输入下一个 prompt 后恢复闪电标记。原始 Tab 颜色、标题和 Badge 在状态切换时保存并恢复。

**使用的 Hook 事件**：`idle_prompt`、`permission_prompt`、`UserPromptSubmit`

**实现架构**：
```
Claude Code hooks → JSON 信号文件 → iTerm2 Python AutoLaunch 脚本（1s 轮询）
```
- Python 脚本通过 iTerm2 的 AutoLaunch 机制自动启动
- 轮询信号文件目录，TTY → Tab 匹配
- 插件首次启动时自动部署 Python 运行时和适配器
- 无屏幕抓取、无正则、无脆弱 hack

**配置**：`/iterm2-tab-status:config` 交互式设置。`~/.config/claude-tab-status/config.json`，支持热重载。环境变量可覆盖各项设置。

---

### 3. burnkit（iTerm2 Tab 颜色组件）

| 属性 | 内容 |
|------|------|
| **名称** | burnkit |
| **作者** | hanzhangzzz |
| **平台** | macOS + iTerm2 |
| **安装方式** | `npm install -g burnkit` → `burnkit install tabs` |
| **版本** | 0.1.2 |
| **来源** | https://github.com/hanzhangzzz/burnkit |

**可视化方式**：

| 颜色 | 含义 | 触发条件 |
|------|------|---------|
| 绿色 | 刚完成，等待你 | Stop hook 立即设置 |
| 黄色 | 等了一会儿 | ~10 分钟空闲 |
| 红色 | 等太久了 | ~20 分钟空闲 |
| 白色 | 活跃/处理中 | 你正在该 Tab 或 Claude 工作中 |

**核心设计理念**：颜色作为"通知徽章"——仅对**非活跃** Tab 着色。当前活跃 Tab 始终白色。时间梯度升级由守护进程驱动。

**使用的 Hook 事件**：`Stop`（立即设绿）、`PreToolUse`（立即重置）

**实现架构**：
```
Claude Code hooks → tab_color_hook.sh → 时间戳文件
                                         ↓
                              macOS launchd → tab_color_daemon.py（1s 轮询）
                                         ↓
                              ANSI 逃逸码（瞬时，单窗格）
                              + iTerm2 Python API（全 Tab，~1s 延迟）
```

- **双通道着色**：ANSI 逃逸码瞬时生效（单窗格）+ iTerm2 API 覆盖全 Tab（含分屏）
- **TTY 发现**：`find_claude_tty()` 沿父进程链搜索，直接写 `/dev/ttysXXX`
- **进程管理**：守护进程由 macOS `launchd` 管理，崩溃/重启后自动恢复
- **会话 ID 处理**：剥离 `w0t1p2:` 前缀提取纯 UUID

**配置**：`~/.claude/hooks/config.sh`，可配阈值（分钟）、RGB 颜色值、轮询间隔。

**注**：burnkit 是更大的工具包（含 Provider Router + Burn AI 燃尽跟踪），Tab 颜色是其三个子系统之一。

---

### 4. claude-needs-input

| 属性 | 内容 |
|------|------|
| **名称** | claude-needs-input |
| **作者** | rickardstureborg (Rich Stureborg) |
| **平台** | macOS + iTerm2 |
| **安装方式** | `bash <(curl -fsSL https://raw.githubusercontent.com/rickardstureborg/claude-needs-input/main/install.sh)` |
| **来源** | https://github.com/rickardstureborg/claude-needs-input |

**可视化方式**：

| 状态 | Tab 效果 |
|------|---------|
| Working（工作中） | 默认（无着色） |
| Blocking（阻塞，需回应） | **橙色脉冲动画** |
| Done（完成，无需回应） | 绿色常亮 |

**独特设计**：用 Haiku 小模型做结束语分类——区分"真正需要回答的问题"（如"Which file should I edit?"）和"结束客套话"（如"Anything else I can help with?"）。

**使用的 Hook 事件**：`Stop`、`Notification`、`UserPromptSubmit`、`PreToolUse`/`PostToolUse`

**实现架构**：
```
Stop hook → classify:
  ├─ 无问号且无请求措辞 → 快速路径 → 绿色（0s）
  └─ 有问号 → claude -p --model haiku --no-session-persistence
       ├─ BLOCKING → notify/pulse.sh（橙色脉冲守护进程）
       └─ CLOSING → 绿色
```

- **脉冲守护进程**：`notify/pulse.sh` 作为独立后台进程运行，持有 Tab tty 的写 fd
- **停止条件**：关闭 Tab 会拆除 pty master → 写失败（EIO）→ 守护进程退出
- **安全机制**：`kill -0` 检查父进程、SessionStart 收割器、6 小时上限
- **分类器递归防护**：`CLAUDE_CLASSIFIER_RUNNING=1` 环境变量

---

### 5. claude-code-tab-title

| 属性 | 内容 |
|------|------|
| **名称** | claude-code-tab-title |
| **作者** | franzvill |
| **平台** | 跨平台（任意支持 OSC 0 的终端） |
| **安装方式** | Claude Code 插件：`/plugin install tab-title@claude-code-tab-title` |
| **版本** | 0.1.0 |
| **来源** | https://github.com/franzvill/claude-code-tab-title |

**可视化方式**：终端 Tab/窗口标题动态更新，含状态标记和会话主题。

| 标记 | 含义 | 示例标题 |
|------|------|---------|
| `*` | 忙碌（刚提交 prompt） | `* fix the login bug` |
| `·` | 空闲（完成回复） | `· fix the login bug` |

**使用的 Hook 事件**：`SessionStart`、`UserPromptSubmit`、`Stop`

**实现架构**：
```
Hook 脚本 → 沿父进程树搜索 pty 设备 → 写 OSC 0 逃逸序列
```

关键技术点：
- **进程树遍历**（最多 10 层）：Linux 读 `/proc/<pid>/stat` 取 `tty_nr`（字段 7），扫描 `/dev/pts/` 找匹配 `st_rdev` 的设备并验证写权限
- **macOS 回退**：`ps -o tty=,ppid= -p $PPID`
- **状态持久化**：`/tmp/claude-tab-<session_id>` JSON 文件
- **去重**：`last_title` 字段防重复写入相同 OSC 序列

**主题提取**：从第一个 prompt 的非空首行提取（最多 500 字节），sticky——会话期间不变。

**已测试终端**：VS Code 集成终端、iTerm2、Terminal.app、Linux `gnome-terminal` 等。

---

### 6. tabby-claude-status

| 属性 | 内容 |
|------|------|
| **名称** | tabby-claude-status |
| **平台** | Windows + Tabby 终端 |
| **安装方式** | Tabby 插件系统 |
| **版本** | 1.2.1 |
| **来源** | https://www.npmjs.com/package/tabby-claude-status（fork 自 tabby-claude-status-gse） |

**可视化方式**（每个表面可独立开关）：

| 表面 | 默认 | 说明 |
|------|------|------|
| Tab 底部边框颜色 | 开 | 按状态变化（working=蓝、question=黄、done=绿、error=红） |
| Tab 标题 emoji 前缀 | 关 | 可配：⚡ ❓ ✅ ❌ |
| 不确定进度条 | 关 | working 状态时脉冲动画 |
| 活动标记点 | 关 | question/error 时显示 Tabby 原生活动点 |
| 任务栏闪烁 | 关 | 仅 Tabby 失焦时 |
| 任务栏图标叠加 | 关 | 16x16 彩色 PNG |

附加：TTS 语音播报（多后端：Web Speech SAPI、Edge TTS、Windows OneCore、Piper）+ 会话恢复 + 麦克风/Zoom 感知静音。

**使用的 Hook 事件**：
| 状态 | Hook 事件 |
|------|----------|
| working | PreToolUse / PostToolUse / UserPromptSubmit |
| question | Notification / PermissionRequest |
| done | Stop |
| error | PostToolUseFailure |
| idle | SessionStart / SessionEnd |

**实现架构**：
```
Claude Code hooks → hook.js → %TEMP%\tabby-claude-status.json
                                    ↓
                          Tabby 插件 watch 文件 → 更新匹配 Tab 的视觉表面
```

- **Tab 匹配**：Windows PID 族谱（沿父进程链最多 6 层）
- **会话恢复**：持久化会话列表，Tabby 重启后自动恢复

---

## 状态栏 / HUD

### 7. claude-hud（jarrodwatts）

| 属性 | 内容 |
|------|------|
| **名称** | claude-hud |
| **作者** | jarrodwatts |
| **平台** | 跨平台（需 Node.js 18+） |
| **安装方式** | Claude Code 插件：`/plugin install claude-hud` |
| **GitHub Stars** | >25k（截至 2026 年中） |
| **来源** | https://github.com/jarrodwatts/claude-hud |

**可视化方式**：利用 Claude Code 原生 `statusLine` API，在输入行下方显示实时 HUD。

显示内容（可配置）：

| 行 | 内容 |
|----|------|
| 默认 | 模型名、项目路径、Git 分支、上下文用量条、API 限额 |
| 可选 | 工具活动（如 `◐ Edit: auth.ts | ✓ Read x3`）、子 Agent 监控（类型、模型、任务描述、耗时）、Todo 进度（来自 TodoWrite）、配置计数、成本、内存 |

**上下文窗口警告**：绿色 <70%、黄色 70-85%、红色 >85%。

**三种预设**：Full / Essential / Minimal。

**使用的 Hook 事件**：不使用 hooks——走 `statusLine` API。

**实现架构**：
```
Claude Code → stdin 传入 JSON（事件驱动 + 300ms 去抖窗口）→ claude-hud 脚本 → stdout → 终端渲染
              ↘ 转录 JSONL（解析工具调用、Agent、Todo）
```

- 零 npm 外部依赖（TypeScript 编译为单文件，运行时仍需 Node.js）
- 使用两个数据源：(1) statusLine API 提供真实 token 数据（非估算）；(2) 解析 transcript JSONL 文件提取工具调用、子代理活动、todo 进度
- 配置：`~/.claude/plugins/claude-hud/config.json`
- 自定义：`/claude-hud:configure` 交互式配置

**已知问题**：Linux `/tmp` 跨设备文件系统错误（设置 `TMPDIR=~/.cache/tmp`）、Windows 需要显式 Node.js LTS、macOS 可能需要直接路径替代动态命令。

---

### 8. spark-hud

| 属性 | 内容 |
|------|------|
| **名称** | spark-hud |
| **平台** | 跨平台（bash + Python 3） |
| **安装方式** | `npx spark-hud` |
| **版本** | 0.7.0 |
| **来源** | https://www.npmjs.com/package/spark-hud |

**可视化方式**：在 Claude Code **每次回复的顶部**显示实时状态行（通过 hook 注入到 context 中）。

显示内容：
- **行 1（始终可见）**：Git 分支、diff 权重、模型、token 数、会话计时器、plant（计划名）
- **行 2（仅告警）**：密钥检测、压缩警告、环境漂移、上次会话、子 Agent、天气、时区
- **静默注入**：已触碰文件、prompt 计数、Todo、已探索目录

**渐进披露**：首次 prompt 显示带标签的完整 HUD，后续 prompt 剥离标签仅显示变化量（delta-only）。

**使用的 Hook 事件**：`UserPromptSubmit`（注入 HUD 到 `additionalContext`）、`Stop`（解析转录取 token/模型/文件）、`PreCompact`（标记压缩事件）

**实现架构**：
```
UserPromptSubmit hook → spark.sh → additionalContext 注入（memory layer 优先级）
Stop hook             → spark-stop.sh → 解析转录 → 状态文件
PreCompact hook       → spark-precompact.sh → 标记压缩
```

关键技术点：
- **注入位置**：通过 `additionalContext` 返回，被放在 memory layer priority（layer 5/6）
- **措辞控制**：directive 措辞 → Claude 逐字复现 HUD；passive 措辞 → Claude 静默吸收
- **自定义 Widget**：`.spark/widgets/*.sh`，需 `SPARK_ENABLE_UNSAFE_CUSTOM_WIDGETS=1`
- **安全设计**：正常执行不发起网络请求，天气需显式 env var 才能启用
- **会话滚动**：30 分钟不活跃后自动滚动

---

### 9. claude-status-bar

| 属性 | 内容 |
|------|------|
| **名称** | claude-status-bar |
| **平台** | 跨平台（需 Node.js） |
| **安装方式** | `npm install -g claude-status-bar` |
| **版本** | 1.3.0 |
| **来源** | https://socket.dev/npm/package/claude-status-bar |

**可视化方式**：Powerline 风格状态栏，通过 `statusLine` API 渲染。

- **10 个内置 Widget**：模型、Git 分支、Token、成本、会话时间、CWD、上下文窗口、Todo 进度、内存用量、文件变更
- **3 种主题**：`powerline-dark`（需 Nerd Font）、`powerline-light`、`minimal`（纯 ASCII）
- **多行支持**（v1.3.0+）：终端宽度不够时自动换行
- **交互式 TUI 配置**：`npx claude-status-bar config`
- **国际化**：英文 / 韩文，自动检测

**使用的 Hook 事件**：走 `statusLine` API（同 claude-hud）

**实现架构**：
```json
// ~/.claude/settings.json
{ "statusLine": { "type": "command", "command": "npx claude-status-bar" } }
```
```text
Claude Code → stdin JSON → 脚本 → stdout（ANSI Powerline 字符）→ 终端渲染
```

---

### 10. @ericcai/claude-statusline

| 属性 | 内容 |
|------|------|
| **名称** | @ericcai/claude-statusline |
| **平台** | 跨平台 |
| **安装方式** | `npx @ericcai/claude-statusline` |
| **版本** | 1.1.0 |
| **来源** | https://www.npmjs.com/package/@ericcai/claude-statusline |

**可视化方式**：另一种 Powerline 风格状态栏，特色功能包括：
- **上下文计量器**：10 格可视化条
- **Git 分支**：含脏标记和 worktree 回退
- **PR 状态徽章**：approved / changes requested / draft / open
- **速率限制用量条**：从 OAuth API 获取（60s 缓存）
- **Vim 模式**和**努力级别**指示器

**使用的 Hook 事件**：走 `statusLine` API

---

## 多会话监控 / TUI 仪表盘

### 11. vibe-term

| 属性 | 内容 |
|------|------|
| **名称** | vibe-term |
| **平台** | 跨平台（需 tmux + Node.js 20+） |
| **安装方式** | `npm install -g vibe-term` |
| **版本** | 1.4.1（2026-02-05） |
| **来源** | https://www.npmjs.com/package/vibe-term |

**可视化方式**：tmux 内固定在顶部的 **HUD 条带**，所有 Claude Code 会话显示为 Tab。

HUD 内容：
- **会话状态**：working / idle / blocked，用文字或颜色标记
- **上下文窗口用量**：绿/黄/红交通灯色
- **会话编号**：`Alt+1` 到 `Alt+9` 即时切换

**操作快捷键**：
| 按键 | 功能 |
|------|------|
| `Alt+1`-`Alt+9` | 切换到第 N 个会话 |
| `Ctrl+H` | 聚焦 HUD 窗格 |
| `n` | 新建会话（含目录 tab 补全） |
| `x` | 终止会话（杀 tmux 窗格 + 清状态） |
| `Ctrl+\` | 从 tmux 会话分离 |

**使用的 Hook 事件**：通过 `vibe-term setup` 安装全局 hooks（具体事件由 hook 脚本定义，用于跟踪 working/idle/blocked 状态）

**实现架构**：
```
vibe-term → 启动 tmux 会话 + HUD 条带（顶部固定窗格）
Claude Code hooks → 状态跟踪 → HUD 进程读取 → 渲染 Tab 行
```

- **会话自动检测**：vibe-term 内启动的会话和外部 tmux 窗格中运行的会话均可检测
- **Hook 冲突管理**：`vibe-term audit` 扫描项目级设置冲突，`vibe-term fix` 合并解决

---

### 12. cctiles

| 属性 | 内容 |
|------|------|
| **名称** | cctiles |
| **作者** | WaTeR-7 |
| **平台** | 跨平台（Rust，crates.io 发布） |
| **安装方式** | `cargo install cctiles` |
| **来源** | https://github.com/WaTeR-7/cctiles |

**可视化方式**：Rust TUI **网格布局**，每个 Tile 运行一个 Claude Code 会话。每个 Tile 内显示：

- **彩色状态指示器**：idle / working / waiting for permission / asking a question / running background task / crashed
- **实时活动摘要**：滚动显示最近工具调用和消息
- **Git 状态**：当前分支 + diffstat（如 `main +12/-3`）

**操作**：
| 按键 | 功能 |
|------|------|
| `h/j/k/l` / 方向键 | 移动焦点 |
| `Enter` | 浮层终端（叠加到网格上） |
| `Ctrl+G` | 从浮层终端回到网格 |
| `r` | 重启会话 |
| `x` | 终止会话 |
| `?` | 快捷键帮助 |
| `q` | 退出 |

**使用的 Hook 事件**：通过 Claude Code hooks 驱动状态监控（非屏幕抓取）

**实现架构**：
```rust
cctiles（Rust TUI）
  ├─ 网格布局引擎 — 每 Tile = 一个 claude 子进程
  ├─ 状态监控 — 从 hooks 输出文件读取
  ├─ 浮层终端 — 按 Enter 进入交互模式
  └─ 配置 — ~/.config/cctiles/config.toml（rows, cols, tile_dirs）
```

**关键设计**：
- 每个 Tile 独立管理自己的 claude 子进程（stdin/stdout/pty）
- 状态由 hooks 驱动而非抓取终端输出——后台任务状态同样准确
- 崩溃恢复：退出的会话显示 in-tile 错误，可按 `r` 原地重启
- **已知限制**：两个 Tile 指向同一目录会互相污染状态

---

### 13. ccmonitor

| 属性 | 内容 |
|------|------|
| **名称** | ccmonitor |
| **作者** | martinwickman |
| **平台** | 跨平台（Go，支持 Windows Terminal + tmux） |
| **安装方式** | `go install github.com/martinwickman/ccmonitor@latest` + Claude Code 插件 |
| **版本** | 0.9.2 |
| **来源** | https://github.com/martinwickman/ccmonitor |

**可视化方式**：终端 UI 列出所有运行中的 Claude Code 会话：

- 按**项目/目录**分组
- 状态：**working / waiting for input / idling**
- 显示最新 prompt 或摘要（`p` 切换）
- **点击切换**：tmux 窗格（Linux/macOS）或 Windows Terminal Tab（Windows）

**使用的 Hook 事件**：通过 Claude Code hooks 写状态文件

**实现架构**：
```
Claude Code hooks → ~/.ccmonitor/ 状态文件 → Go TUI 读取 → 渲染会话列表
```

- **双组件架构**：Hook 层（数据采集）+ Monitor 层（显示）
- **平台原生后端**：
  - Linux/macOS：tmux 集成
  - Windows：UI Automation 查找并切换 Windows Terminal Tab（`internal/wt` 包）
- **死会话清理**：自动清理（有时不稳定）+ 手动 `ccmonitor --clean`

---

### 14. joy（@yumazak/joy）

| 属性 | 内容 |
|------|------|
| **名称** | @yumazak/joy |
| **作者** | yumazak |
| **平台** | 跨平台（需 Node.js） |
| **安装方式** | `npm i -g @yumazak/joy` → `joy` |
| **版本** | 0.2.10 |
| **来源** | https://www.npmjs.com/package/@yumazak/joy |

**可视化方式**：TUI 仪表盘，显示所有活跃 Claude Code 会话状态。

| 状态 | 指示器 | 含义 |
|------|--------|------|
| Processing | 🔄 | Claude 工作中 |
| WaitingApproval | 🟡 | 等待工具使用审批 |
| WaitingInput | 🟢 | 等待用户输入 |

**使用的 Hook 事件**：通过 joy-hooks 插件安装。`PostToolUse` → Processing、`PermissionRequest` → WaitingApproval、`Stop`/`UserPromptSubmit` → WaitingInput。

**实现架构**：
```
joy 启动本地 HTTP 服务器（127.0.0.1:50055）
joy-hooks 插件 → Claude Code hooks → 状态上报 → HTTP API → TUI 仪表盘
```

- **零噪声**：Joy 未运行时不产生任何错误/输出
- **可选端口**：`--port` / `-p` 或 `JOY_PORT` 环境变量

---

## Agent 专用工具

### 15. agentflow（@1yoouoo/agentflow）

| 属性 | 内容 |
|------|------|
| **名称** | @1yoouoo/agentflow |
| **平台** | 跨平台 |
| **安装方式** | `npm install -g @1yoouoo/agentflow` → `agentflow setup` |
| **版本** | 0.1.0 |
| **来源** | https://www.npmjs.com/package/@1yoouoo/agentflow |

**可视化方式**：利用 Claude Code 原生 `statusLine` 显示 Agent 编排状态栏。

示例：`⠙ batch-orchestrator > video-converter  ━━━━━━░░░░  3/8  37%  1m 23s`

包含：
- **动画旋转器**：指示活跃状态
- **Agent 执行路径**：`root > parent > child` 父子关系链
- **进度条**：完成数/总数 + 百分比
- **已用时间**

**使用的 Hook 事件**：`PreToolUse` / `PostToolUse`（仅针对 `Agent` 工具）

**实现架构**：
```
Claude Code hooks → ~/.cc-flow/events.jsonl → statusLine 脚本读取 → 渲染状态栏
```

- **父子关系推断**：基于调用深度自动推断嵌套关系
- **额外命令**：`agentflow watch`（查看完整 Agent 树）、`agentflow clear`（清除历史）
- **许可**：MIT

---

## VS Code 集成

### 16. cc-hud

| 属性 | 内容 |
|------|------|
| **名称** | cc-hud |
| **作者** | stelcart |
| **平台** | VS Code 扩展 |
| **来源** | https://github.com/stelcart/cc-hud |

**可视化方式**：VS Code 扩展，提供四窗格 HUD：

| 窗格 | 内容 |
|------|------|
| Todo 树 | 任务进度 |
| Plan 视图 | 计划展示（含交互式复选框） |
| Activity 日志 | 实时活动记录（支持自动跟随和搜索） |
| Context 追踪器 | 上下文用量 |

**使用的 Hook 事件**：`sync-plan.js`、`log-activity.js`、`sync-context.js`、`pre-compact.js`（通过 hooks → 文件同步）

**实现架构**：
```
Claude Code hooks → 文件同步 → VS Code 扩展读取 → Webview 渲染
```

- 真实 token 用量（从 API 获取）
- 状态栏显示上下文用量百分比

---

## 其他值得关注的项目

### 17. claude-wsl-integration

| 属性 | 内容 |
|------|------|
| **名称** | @fullstacktard/claude-wsl-integration |
| **平台** | Windows Terminal + WSL |
| **安装方式** | `npm install -g fullstacktard/claude-wsl-integration` |
| **来源** | https://socket.dev/npm/package/@fullstacktard/claude-wsl-integration |

**可视化方式**：Windows Terminal Tab 指示器。
- 橙色圆点/旋转器 Tab 指示器
- 铃铛 emoji + Windows Toast 通知（回复就绪时）
- 目录继承（新 Tab 自动继承当前工作目录）

**使用的 Hook 事件**：`SessionStart`、`UserPromptSubmit`、`Stop`

---

### 18. custom-statusline（Agent Skill）

| 属性 | 内容 |
|------|------|
| **名称** | custom-statusline |
| **类型** | Claude Code Agent Skill（非独立工具） |
| **来源** | https://skillsmp.com/zh/creators/zeulewan/claude-code-skills/custom-statusline |

**可视化方式**：通过 `statusLine` API + bash 脚本 + Nerd Font 图标显示综合状态行。

包含：Git 信息、API 用量（5h/7d）、Token 显示、项目上下文（Python/Node/Docker 版本）、系统信息（电池/WiFi）、成本/耗时。

---

## Claude Code statusLine API 架构

（供 slTerminal 设计自身 hooks 可视化系统参考）

### 渲染管道

```
Claude Code (Ink)                              用户脚本
─────────────────                              ──────────
buildStatusLineCommandInput()
  → 收集运行时状态（模型、token、成本、
     上下文窗口、Git、Vim 模式等）
  → 序列化为 JSON

executeStatusLineCommand()
  → stdin 管道传入 JSON ───────────────────→ 脚本收到 JSON
  → 读取 stdout ←─────────────────────────── 脚本输出文本

setAppState({ statusLineText })
  → Zustand store, memo 组件重渲染

<StatusLine /> → <Text><Ansi>{text}</Ansi></Text>
```

### 输入协议（stdin JSON 字段）

| 字段 | 说明 |
|------|------|
| `model.id` / `model.display_name` | 当前模型 |
| `workspace.current_dir` / `project_dir` | 工作目录 |
| `cost.total_cost_usd` / `total_duration_ms` | 会话成本与耗时 |
| `context_window.current_usage` | Token 用量（输入、输出、缓存） |
| `context_window.used_percentage` | 0-100 使用百分比 |
| `context_window.context_window_size` | 模型上下文限制 |
| `rate_limits.five_hour` / `seven_day` | 速率限制信息 |
| `vim.mode` | INSERT / NORMAL 等 |
| `session_id` | UUID |

### 输出协议

- stdout 经 trim → 按 `\n` 分割 → 空行丢弃 → 重新拼接
- **ANSI SGR 序列保留**，经 Ink `<Ansi>` 组件解析
- `exit 0` + 有 stdout → 显示；`exit 0` + 空 stdout → 清除行；非零退出 → 忽略
- **超时**：默认 5000ms，超时忽略结果
- **单次飞行**：新触发中止正在运行的 shell 进程

### 触发源

1. **事件驱动**：新 assistant 消息、权限模式变化、Vim 模式切换、模型切换
2. **设置驱动**：`settings.statusLine.command` 变更时
3. **时间驱动**：`refreshInterval`（秒），0 或缺失则禁用定时器

全部走 **300ms 去抖队列**（`scheduleUpdate`）。

### 安全网关（3 层）

1. 托管设置全局禁用所有 hooks
2. 工作区信任检查——未接受信任对话框则跳过脚本
3. 仅允许托管 hooks

---

## 分类对比矩阵

| 工具 | 终端平台 | 可视化介质 | 集成方式 | Hook 事件 | 实现语言 | 多会话 |
|------|---------|-----------|---------|----------|---------|--------|
| **claude-iterm2** | iTerm2 (macOS) | Tab 颜色 + Badge + 动画 | Hooks → ANSI/API | SessionStart, Stop, PreToolUse, PermissionRequest | JS (预打包) | 是（多 Tab） |
| **claude-code-iterm2-tab-status** | iTerm2 (macOS) | Tab 闪烁 + Badge | Hooks → JSON 文件 → Python 轮询 | idle_prompt, permission_prompt, UserPromptSubmit | Python | 是（多 Tab） |
| **burnkit (tabs)** | iTerm2 (macOS) | Tab 颜色（时间梯度） | Hooks → 时间戳 → launchd 守护进程 | Stop, PreToolUse | Bash + Python | 是（多 Tab） |
| **claude-needs-input** | iTerm2 (macOS) | Tab 脉冲 + 颜色 | Hooks → Haiku 分类 → 脉冲守护进程 | Stop, Notification, UserPromptSubmit, PreToolUse | Bash | 是（多 Tab） |
| **claude-code-tab-title** | 任意终端 | Tab/窗口标题 + 标记 | Hooks → 进程树 → OSC 0 序列 | SessionStart, UserPromptSubmit, Stop | Python | 否 |
| **tabby-claude-status** | Tabby (Windows) | Tab 边框颜色 + 进度条 + 活动点 | Hooks → JSON 文件 → 插件轮询 | PreToolUse, PostToolUse, Stop, Notification, PermissionRequest 等 | JS (hook.js) | 是（多 Tab） |
| **claude-hud** | 任意终端 | statusLine（输入行下方） | statusLine API | 无（不依赖 hooks） | TypeScript | 否 |
| **spark-hud** | 任意终端 | 回复顶部状态行（context 注入） | Hooks → additionalContext | UserPromptSubmit, Stop, PreCompact | Bash + Python 3 | 否 |
| **claude-status-bar** | 任意终端 | Powerline 状态栏 | statusLine API | 无 | Node.js | 否 |
| **@ericcai/claude-statusline** | 任意终端 | Powerline 状态栏 | statusLine API | 无 | Node.js | 否 |
| **vibe-term** | tmux (跨平台) | HUD 条带（顶部固定） | Hooks | 会话状态跟踪（setup 安装） | Node.js | 是（核心功能） |
| **cctiles** | 跨平台 | TUI 网格（每 Tile = 一个会话） | Hooks → 文件 | Claude Code hooks | Rust | 是（核心功能） |
| **ccmonitor** | 跨平台 | TUI 会话列表 | Hooks → 文件 | Claude Code hooks | Go | 是（核心功能） |
| **joy** | 跨平台 | TUI 仪表盘 | Hooks → HTTP API | PostToolUse, PermissionRequest, Stop, UserPromptSubmit | Node.js | 是（核心功能） |
| **agentflow** | 跨平台 | Agent 编排 statusLine | Hooks → JSONL → statusLine | PreToolUse, PostToolUse (Agent) | Node.js | 否 |
| **cc-hud** | VS Code | 四窗格 Webview | Hooks → 文件同步 | sync-plan, log-activity, sync-context | TypeScript | 否 |
| **claude-wsl-integration** | Windows Terminal + WSL | Tab 指示器 + Toast | Hooks | SessionStart, UserPromptSubmit, Stop | Node.js | 否 |

---

## 对 slTerminal 的启示

1. **双路径共存**：社区工具分两大阵营——`statusLine` API（内置状态栏）和 Hooks 事件（外部视觉反馈）。slTerminal 作为终端模拟器本身，两种均可支持，但各有侧重：
   - `statusLine` 适合在终端视口内渲染（如底部状态栏）
   - Hooks 路径适合在终端 chrome 层渲染（如 Tab 颜色、Badge、任务栏进度条）

2. **Tab 视觉反馈是最主流的外部可视化方式**：7 个工具（claude-iterm2、claude-code-iterm2-tab-status、burnkit、claude-needs-input、claude-code-tab-title、tabby-claude-status、claude-wsl-integration）都选择 Tab 作为视觉表面——因为它是用户切换会话时最先看到的 UI。

3. **颜色状态映射存在广泛共识但不绝对**（不同工具存在差异——如 burnkit 以白色表示活跃、时间梯度方案替代固定颜色映射；claude-code-tab-title 仅用符号标记不使用颜色）：
   - 蓝/白 = 工作中
   - 黄/橙 = 需关注（权限/输入等待）
   - 绿 = 完成
   - 红 = 错误

4. **进度条/脉冲动画是"工作中"状态的有效补充**：仅靠颜色不足以区分"空闲"和"工作中"，不确定进度条（tabby-claude-status）或脉冲动画（claude-needs-input）是低认知负载的有效方案。

5. **多会话监控是独立赛道**：vibe-term、cctiles、ccmonitor、joy 都是独立的 TUI 应用，面向同时管理多个 Claude Code 会话的用户。slTerminal 本身即是此类宿主（Dockview 多面板），天然的差异化优势。

6. **Hook → 文件 → 轮询 是最通用的架构**：因为 hook 脚本无 controlling tty，写文件 + 外部进程（或终端模拟器自身插件）轮询是唯一跨平台通用的方案。

7. **进程树遍历是直达终端的高效捷径**：claude-code-tab-title 和 burnkit 通过解析 `/proc/<pid>/stat`（Linux）或 `ps`（macOS）找到父进程的 pty 设备并直接写 ANSI/OSC 序列——省去中间文件层但高度平台相关。

---

*调研完成。所有信息截至 2026-07-25。*
