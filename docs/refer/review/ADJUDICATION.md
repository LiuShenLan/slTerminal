# 跨文件矛盾裁决

> 裁决日期: 2026-08-15
> 依据: verify-research-output skill 证据层级（官方文档 > 源码/结构化数据端点 > 社区权威来源）
> 检测: detect-conflicts.mjs 确定性脚本，输入 12 条 claims，输出 6 组冲突（均为同一文件内部矛盾，无跨文件冲突——三组核查子代理的交叉比对结论与此一致）

## 冲突 1: VS Code 右键菜单背景色（vscode.md 行 211 vs 行 240）

| 方向 | 声称 |
|------|------|
| devtools | `#3C3C3C`（行 211，4.2 组件拆解） |
| devtools | `#252526`（行 240，5.2 Dark+ JSON 节选） |

**裁决**: 行 240 正确。dark_vs.json 原文 `"menu.background": "#252526"`（已独立抓取逐字确认）；`#3C3C3C` 为 input.background 值，系字段混淆。
**正确信息**: 右键菜单背景 `#252526`（Dark+）。
**依据**: https://raw.githubusercontent.com/microsoft/vscode/main/extensions/theme-defaults/themes/dark_vs.json
**严重程度**: P2

## 冲突 2: VS Code 右键菜单选中色（vscode.md 行 211 vs 行 53）

| 方向 | 声称 |
|------|------|
| devtools | `#04395E`（行 211） |
| devtools | `#0078d4`（行 53，2.1.3 表） |

**裁决**: 行 53 正确。dark_vs.json 显式覆盖为 `#0078d4`；`#04395E` 仅为 menuColors.ts 注册默认（被 Dark+ 覆盖）。
**正确信息**: 选中项 `#0078d4`（Dark+ 显式值）；分隔线 `#454545`（不透明）。
**依据**: dark_vs.json + src/vs/platform/theme/common/colors/menuColors.ts
**严重程度**: P2

## 冲突 3: macOS 动态系统色数量（apple.md 行 17 vs 行 21–24）

| 方向 | 声称 |
|------|------|
| design | "38 个动态系统色" |
| design | 四类分类表恰好 35 项 |

**裁决**: 35 正确。官方 color.json macOS 表 35 个数据行（35/35 名称逐一确认存在）。
**正确信息**: macOS 定义 35 个动态系统色。
**依据**: https://developer.apple.com/tutorials/data/design/human-interface-guidelines/color.json
**严重程度**: P2

## 冲突 4: Zed 语义状态色组数（zed.md 行 207）

| 方向 | 声称 |
|------|------|
| devtools | "13 组 ×3 级" |
| devtools | 同段枚举 14 个名称 |

**裁决**: 14 正确。one.json style 下 14 个语义状态色键全部存在、无缺失。
**正确信息**: 语义状态色 14 组 ×3 级。
**依据**: https://raw.githubusercontent.com/zed-industries/zed/main/assets/themes/one/one.json
**严重程度**: P2

## 冲突 5: Sublime 默认配色方案（devtools-other.md 行 15 两句）

| 方向 | 声称 |
|------|------|
| devtools | "默认配色方案为 Monokai（官方文档原文）[2]" |
| devtools | "ST4 将 Mariana 列为首推暗色方案[6]" |

**裁决**: 两句均为真实引文但存在张力：ST4 官方博客 changelog 明示「Changed default color scheme to Mariana」（已独立确认），即 ST4 起默认已改 Mariana；color_schemes.html 的 "the default Monokai color scheme" 为现行文档表述。归因 [6] 错误（[6] 是终端配色移植仓库，不含 ST4 内容），正源为 ST4 博客 [4]。
**正确信息**: ST4 起默认配色方案改为 Mariana（ST4 官方博客 changelog）；现行官方 color_schemes 文档仍称 "the default Monokai color scheme"，两处来源并存如实记录。
**依据**: https://www.sublimetext.com/blog/articles/sublime-text-4（"Changed default color scheme to Mariana"）
**严重程度**: P1（归因错误 + 内部张力）

## 冲突 6: Notion 边框原生值（notion-figma.md 行 26 vs 行 165）

| 方向 | 声称 |
|------|------|
| design | §2.1 边框原生值 `rgb(37,37,37)` |
| design | §5.1 映射表 `rgb(32,32,32) → rgb(37,37,37)` |

**裁决**: 行 165 正确。global.css 原文 `border: rgb(32,32,32) => rgb(37, 37, 37)`（已独立抓取确认）——原生值为 rgb(32,32,32)，替换后为 rgb(37,37,37)。
**正确信息**: 边框原生值 rgb(32,32,32)（暗色替换为 rgb(37,37,37)）。
**依据**: https://notionthemes.netlify.app/dark/global.css
**严重程度**: P2
