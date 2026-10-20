# 功能规格资产

> 目标态功能规格使用的 CLI logo、界面图标与应用图标；记录来源、许可、显示尺寸、正文引用与使用位置。

## 目录说明

- `icons/`：21 个界面 SVG 图标；其中 16 个来自 lucide-static，2 个为项目自绘文件图标，3 个为项目自绘窗口控制图标。
- `cli-svg/`：22 个 CLI 品牌 SVG logo，原样复制。
- `brand/`：`app-icon.png`，slTerminal 应用图标，256×256。
- `preview/`：仅供文档深色预览使用的副本；原文件保持原样。
- `LICENSE-lucide.txt`：`lucide-static@1.31.0` 上游许可原文（ISC License，含 Feather 派生图标的 MIT 段落）。

## 许可与来源

### 界面图标

- 16 个 lucide 图标来自 `lucide-static@1.31.0`，许可为 ISC；其中从 Feather 派生的图标同时适用 MIT。完整文本见 `LICENSE-lucide.txt`。
- 图标内容与上游一致，仅按文档语义重命名文件；均为 24×24 viewBox、`currentColor` 描边。
- `file-generic.svg` 与 `file-generic-typed.svg` 为项目自绘，许可随项目。
- `window-minimize.svg`、`window-maximize.svg`、`window-restore.svg` 为项目自绘窗口控制图标，风格对齐 lucide（24×24 viewBox、2px 描边、`currentColor`），许可随项目。
- `file-generic-typed.svg` 左缘类型色块以 `currentColor` 占位，实际颜色按扩展名分组，由实现侧注入。

### CLI logo

- 22 个 SVG 由项目作者提供，作为开源 CLI 品牌标识参考收录；文件本身不含许可证头。
- 文件内嵌 id 显示部分素材源自 Devicon 与 Lobe Icons 生态；未取得逐文件授权确证，因此不在此虚构具体许可证。
- 各 CLI 名称与图形商标归各自权利人所有；此处仅用于识别与说明，不构成商标授权。
- `deepseek.svg` 对应套餐余量来源 DeepSeek，不是 `AgentKind`。

### 应用图标

- `brand/app-icon.png` 与仓库根 `assets/app-icon.png` 为同一图形的 256×256 版本；规格在标题栏处引用。

## 预览规则

`preview/` 只服务文档内嵌显示，不替代原文件。生成规则一次性执行，不保留脚本：

- `preview/icons/`：21 个文件全部生成。在根 `<svg>` 增加 `color="#E6E6E6"`，保留 `currentColor` 与路径数据，使描边在暗色背景可读。
- `preview/cli-svg/`：只为纯黑/深灰墨色文件生成。
  - `grok.svg`、`kilocode.svg`、`pi.svg`：根 `color` 改为 `#E6E6E6`。
  - `cline.svg`、`codex.svg`、`github-copilot.svg`：`#000` / `#000000` → `#E6E6E6`。
  - `goose.svg`：`#101010` → `#E6E6E6`。
  - `opencode.svg`：`#211E1E` → `#E6E6E6`，`#CFCECD` → `#3A3A3A`。
  - `cursor.svg`：按亮度反相，`#43413C` → `#E6E6E6`、`#55544F` → `#C8C8C8`、`#72716D` → `#A8A8A8`、`#D6D5D2` → `#5A5A5A`、`#FFFFFF` → `#3A3A3A`；亮度遮罩内的白色保持不变。
- 彩色 logo 不生成预览，正文直接引用 `cli-svg/` 原文件。

## 界面图标清单

| 文件 | 上游 / 来源 | 许可 | 显示尺寸 | 正文引用 | 使用位置 |
|---|---|---|---|---|---|
| `icons/activity-nav.svg` | lucide `folder-tree` | ISC | 15px | preview | `sidebar/framework.md` · 视图集合与注册 |
| `icons/activity-explorer.svg` | lucide `folder` | ISC | 15px | preview | `sidebar/framework.md` · 视图集合与注册 |
| `icons/activity-commit.svg` | lucide `git-branch` | ISC | 15px | preview | `sidebar/framework.md` · 视图集合与注册 |
| `icons/activity-agent-files.svg` | lucide `bot` | ISC | 15px；14px（Agent 节点） | preview | `sidebar/framework.md` · 视图集合与注册；`sidebar/agent-files.md` · Agent 节点 |
| `icons/activity-config.svg` | lucide `settings` | ISC | 15px | preview | `sidebar/framework.md` · 视图按钮与配置钮 |
| `icons/alert-triangle.svg` | lucide `triangle-alert` | ISC + MIT | 15px | preview | `workspace/navigation.md` · 界面结构与行；`sidebar/explorer.md` · Git 状态 |
| `icons/chevron-down.svg` | lucide `chevron-down` | ISC + MIT | 12px | preview | `sidebar/explorer.md` · 行构成与视觉；`sidebar/commit.md` · 列表内容与计数；`sidebar/agent-files.md` · 入口、出口与焦点 |
| `icons/chevron-right.svg` | lucide `chevron-right` | ISC + MIT | 12px | preview | `sidebar/explorer.md` · 行构成与视觉；`sidebar/commit.md` · 列表内容与计数；`sidebar/agent-files.md` · 入口、出口与焦点 |
| `icons/close.svg` | lucide `x` | ISC + MIT | 14px | preview | `workspace/shell.md` · 面板标题与页签呈现、窗口、标题栏与单实例；`sidebar/explorer.md` · 树根与界面 |
| `icons/empty-box.svg` | lucide `folder-open` | ISC | 15px | preview | `workspace/shell.md` · 空态；`sidebar/agent-files.md` · 状态与错误反馈 |
| `icons/file-generic.svg` | 项目自绘 FileIcon | 随项目 | 14px | preview | `panels/gitshow.md` · 标题；`sidebar/explorer.md` · 行构成与视觉 |
| `icons/file-generic-typed.svg` | 项目自绘 FileIcon | 随项目 | 14px | preview | `workspace/shell.md` · 面板标题与页签呈现；`panels/gitshow.md` · 标题；`sidebar/explorer.md` · 行构成与视觉；`sidebar/commit.md` · 列表内容与计数 |
| `icons/folder.svg` | lucide `folder` | ISC | 14px | preview | `workspace/navigation.md` · 界面结构与行；`sidebar/explorer.md` · 行构成与视觉；`sidebar/commit.md` · 列表内容与计数 |
| `icons/history.svg` | lucide `clock` | ISC + MIT | 14px | preview | `workspace/navigation.md` · 界面结构与行 |
| `icons/page.svg` | lucide `file-text` | ISC | 14px | preview | 当前规格未引用 |
| `icons/plus.svg` | lucide `plus` | ISC + MIT | 12px | preview | 当前规格未引用 |
| `icons/refresh.svg` | lucide `refresh-cw` | ISC | 13px；14px（Agent 全局文件，示意） | preview | `workspace/navigation.md` · 界面结构与行；`sidebar/agent-files.md` · 入口、出口与焦点 |
| `icons/search.svg` | lucide `search` | ISC + MIT | 12px | preview | `workspace/navigation.md` · 界面结构与行 |
| `icons/window-minimize.svg` | 项目自绘窗口控制 | 随项目 | 12px | preview | `workspace/shell.md` · 窗口、标题栏与单实例 |
| `icons/window-maximize.svg` | 项目自绘窗口控制 | 随项目 | 12px | preview | `workspace/shell.md` · 窗口、标题栏与单实例 |
| `icons/window-restore.svg` | 项目自绘窗口控制 | 随项目 | 12px | preview | `workspace/shell.md` · 窗口、标题栏与单实例 |

## CLI logo 清单

CLI logo 族在以下位置使用：

- `workspace/shell.md` · 面板标题与页签呈现（页签，16×16px）
- `workspace/navigation.md` · 界面结构与行（活跃/历史会话行，14×14px）
- `panels/terminal.md` · 面板构成与页签
- `panels/settings.md` · 左导航与配置页目录（14×14）
- `sidebar/agent-files.md` · Agent 节点（14px，示意）

正文只嵌入 `claude.svg` 作为家族示例；实现按 `AgentKind` 取对应文件。

| 文件 | AgentKind slug | 显示名 | 许可 / 来源 | 备注 |
|---|---|---|---|---|
| `cli-svg/amp.svg` | `amp` | Amp | 作者提供；Devicon 生态 | — |
| `cli-svg/antigravity.svg` | `antigravity` | Antigravity | 作者提供；Devicon 生态 | 多色 |
| `cli-svg/claude.svg` | `claude` | Claude Code | 作者提供 | 家族示例 |
| `cli-svg/cline.svg` | `cline` | Cline | 作者提供 | 单色；使用预览 |
| `cli-svg/codebuddy.svg` | `codebuddy` | CodeBuddy Code | 作者提供；Lobe Icons 生态 | 多色 |
| `cli-svg/codex.svg` | `codex` | Codex | 作者提供 | 单色；使用预览 |
| `cli-svg/cursor.svg` | `cursor` | Cursor Agent | 作者提供；Devicon 生态 | 灰阶；使用预览 |
| `cli-svg/deepseek.svg` | — | DeepSeek | 作者提供；Devicon 生态 | 套餐余量来源，非 AgentKind |
| `cli-svg/devin.svg` | `devin` | Devin | 作者提供 | 多色 |
| `cli-svg/gemini.svg` | `gemini` | Gemini | 作者提供；Lobe Icons 生态 | 多色 |
| `cli-svg/github-copilot.svg` | `copilot` | GitHub Copilot | 作者提供 | 单色；使用预览 |
| `cli-svg/goose.svg` | `goose` | Goose | 作者提供 | 单色；使用预览 |
| `cli-svg/grok.svg` | `grok` | Grok | 作者提供 | `currentColor`；使用预览 |
| `cli-svg/hermes.svg` | `hermes` | Hermes | 作者提供 | 双色 |
| `cli-svg/kilocode.svg` | `kilo` | Kilo Code | 作者提供 | `currentColor`；使用预览 |
| `cli-svg/kimi.svg` | `kimi` | Kimi Code | 作者提供 | 多色；亦用于套餐余量页脚 |
| `cli-svg/kiro.svg` | `kiro` | Kiro | 作者提供 | 多色 |
| `cli-svg/opencode.svg` | `opencode` | OpenCode | 作者提供 | 灰阶；使用预览 |
| `cli-svg/pi.svg` | `pi` | Pi | 作者提供 | `currentColor`；使用预览 |
| `cli-svg/qoder.svg` | `qodercli` | Qoder | 作者提供 | 双色 |
| `cli-svg/qwen.svg` | `qwen` | Qwen Code | 作者提供 | 单色 |
| `cli-svg/trae.svg` | `trae-cli` | Trae CLI | 作者提供 | 单色 |

## 缺失的品牌资产

以下 `AgentKind` 没有对应 logo；规格中只显示名称或标题，不显示图片：

| AgentKind slug | 显示名 |
|---|---|
| `aider` | Aider |
| `droid` | Droid |
| `auggie` | Auggie |
| `vibe` | Vibe |
| `omp` | Oh My Pi |
| `maki` | Maki |

## 应用图标

| 文件 | 来源 | 尺寸 | 使用位置 |
|---|---|---|---|
| `brand/app-icon.png` | 仓库根 `assets/app-icon.png`（同图 256×256） | 256×256 PNG | `workspace/shell.md` · 窗口、标题栏与单实例（标题栏应用标识，示意 16px） |
