# 侧栏资源

本目录收录侧栏文档使用的品牌位图与界面图标。

## 目录说明

- `brand/`：品牌位图（PNG，原字节复制，未重新压缩）。
- `icons/`：侧栏界面 SVG 图标，含 lucide 导出图标与 FileIcon 自绘文件图标。
- `LICENSE-lucide.txt`：`lucide-static@1.31.0` 包内 LICENSE 原文（ISC License）。

## 品牌位图

| 目标文件 | 来源 | 尺寸/格式 | 侧栏使用位置 | 备注 |
|---|---|---|---|---|
| `brand/claude.png` | `public/cli-icons/claude.png` | 32×32 PNG，界面显示 14×14 | 导航树活跃会话行、历史行、Agent 全局文件节点 | Claude Code 品牌标识 |
| `brand/deepseek.png` | `public/plan-icons/deepseek.png` | 32×32 PNG，界面显示 14×14 | 导航树套餐余量页脚 | DeepSeek 套餐标识 |
| `brand/kimi.png` | `public/plan-icons/kimi.png` | 32×32 PNG，界面显示 14×14 | 导航树套餐余量页脚 | Kimi 套餐标识 |
| `brand/app-icon.png` | `public/app-icon.png` | 64×64 PNG | 应用图标/favicon；侧栏界面不直接使用 | 按用户要求收录 |
| `brand/claude-logo.png` | `src/assets/claude-logo.png` | 20×20 PNG | 当前未被侧栏使用 | 按用户要求作为资料收录并标注 |

## 界面图标

lucide 图标来源于 `lucide-static@1.31.0`，仅重命名文件，内容未改动；均为 24×24 viewBox、`currentColor` 描边。FileIcon 两项为项目自绘。

### 活动栏

| 目标文件 | 来源 | 尺寸/格式 | 侧栏使用位置 | 备注 |
|---|---|---|---|---|
| `icons/activity-nav.svg` | `lucide-static` `folder-tree.svg` | 24×24 viewBox SVG，显示 15px | 活动栏：导航树 | — |
| `icons/activity-explorer.svg` | `lucide-static` `folder.svg` | 24×24 viewBox SVG，显示 15px | 活动栏：文件浏览器 | — |
| `icons/activity-commit.svg` | `lucide-static` `git-branch.svg` | 24×24 viewBox SVG，显示 15px | 活动栏：Commit | — |
| `icons/activity-agent-files.svg` | `lucide-static` `bot.svg` | 24×24 viewBox SVG，显示 15px | 活动栏：Agent 全局文件 | — |
| `icons/activity-config.svg` | `lucide-static` `settings.svg` | 24×24 viewBox SVG，显示 15px | 活动栏底部：配置 | — |

### 树/列表与通用控件

| 目标文件 | 来源 | 尺寸/格式 | 侧栏使用位置 | 备注 |
|---|---|---|---|---|
| `icons/chevron-right.svg` | `lucide-static` `chevron-right.svg` | 24×24 viewBox SVG，显示 12px | 导航树、文件树、Commit 文件列表的折叠节点 | 展开箭头 |
| `icons/chevron-down.svg` | `lucide-static` `chevron-down.svg` | 24×24 viewBox SVG，显示 12px | 导航树、文件树、Commit 文件列表的展开节点 | 收起箭头 |
| `icons/refresh.svg` | `lucide-static` `refresh-cw.svg` | 24×24 viewBox SVG，显示 13px | 导航树刷新按钮 | — |
| `icons/search.svg` | `lucide-static` `search.svg` | 24×24 viewBox SVG，显示 12px / 15px | 导航树搜索框（12px）、空态提示（15px） | — |
| `icons/history.svg` | `lucide-static` `clock.svg` | 24×24 viewBox SVG，显示 14px | 导航树历史会话行 | — |
| `icons/page.svg` | `lucide-static` `file-text.svg` | 24×24 viewBox SVG，显示 14px | 导航树操作页面行 | — |
| `icons/folder.svg` | `lucide-static` `folder.svg` | 24×24 viewBox SVG，显示 14px | 导航树项目行、文件树文件夹行 | 与 `activity-explorer.svg` 同形，保留两份 |
| `icons/empty-box.svg` | `lucide-static` `folder-open.svg` | 24×24 viewBox SVG，显示 15px | 空文件树、无搜索结果等空态 | — |
| `icons/alert-triangle.svg` | `lucide-static` `triangle-alert.svg` | 24×24 viewBox SVG，显示 15px | 文件浏览器的警告提示 | — |
| `icons/close.svg` | `lucide-static` `x.svg` | 24×24 viewBox SVG，显示 14px | 文件浏览器错误横幅关闭 | — |
| `icons/plus.svg` | `lucide-static` `plus.svg` | 24×24 viewBox SVG，显示 12px | 导航树新建按钮 | — |

### FileIcon 自绘

| 目标文件 | 来源 | 尺寸/格式 | 侧栏使用位置 | 备注 |
|---|---|---|---|---|
| `icons/file-generic.svg` | `src/features/explorer/FileIcon.tsx`（自绘） | 14×14 viewBox SVG | 文件树默认文件行 | 无类型色块，颜色经 `currentColor` 注入 |
| `icons/file-generic-typed.svg` | `src/features/explorer/FileIcon.tsx`（自绘） | 14×14 viewBox SVG | 文件树带类型标识的文件行 | 含左侧类型色块；色块以 `currentColor` 占位，实际颜色按扩展名分组，不在文档中写色值 |


## 导出规范

- PNG 一律原字节复制，不重新压缩、不转换格式。
- SVG 保留原 `viewBox` 与 `currentColor`，不写死主题色值。
- lucide 图标内容与上游一致，仅按文档语义重命名文件。
