# linear 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: design

---

## 错误 1: 状态色「成功 #27A644 / #10B981」归因 [6][7]，但 [6] 的成功色是 #4cb782

- **文件+行号**: `docs/refer/linear.md` (行 50)
- **原声称**: "**状态色**：成功 `#27A644` / `#10B981`、告警 `#F2994A`、错误 `#EB5757` [6][7]。"
- **错误类型**: 来源不支撑（归因错误）
- **正确信息**: #27A644/#10B981 仅由 [7]（sweetkey/awesome-design-md DESIGN.md：「Success Green (#27a644)、Emerald (#10b981)」）支撑；[6] designsystems.one 的成功色为 `--linear-success: #4cb782`。告警 #F2994A 与错误 #EB5757 两值在 [6] 中存在（--linear-warning/--linear-error），归因无误。
- **反证来源**: https://www.designsystems.one/design-systems/linear（2026-08-15 抓取）——Palette 09 表「Success Green `#4cb782` Completed states --linear-success」；https://raw.githubusercontent.com/sweetkey/awesome-design-md/main/design-md/linear.app/DESIGN.md——「**Green** (`#27a644`): Primary success/active status」「**Emerald** (`#10b981`): Secondary success — pill badges, completion states」。

## 错误 2: 按钮按压「~100ms ease-in」在 [11] 中不存在

- **文件+行号**: `docs/refer/linear.md` (行 111)
- **原声称**: "| 按钮按压 | ~100ms | ease-in，`scale(0.97)` | :active 按下 | [11] |"
- **错误类型**: 来源不支撑
- **正确信息**: [11]（kylezantos/design-motion-principles 的 emil-kowalski.md + motion-cookbook.md）对按钮按压只有 `button:active { transform: scale(0.97); }`（cookbook §10 "Scale on Press"），全文无 100ms、无 ease-in 数值/曲线。100ms 与 ease-in 的来源需另行定位（可能为其他转述，但不在 [11] 中）。
- **反证来源**: https://raw.githubusercontent.com/kylezantos/design-motion-principles/main/skills/design-motion-principles/references/emil-kowalski.md（第 1 条技巧「Scale your buttons — Subtle scale(0.97) on :active」）与同仓库 references/motion-cookbook.md（§10「### Scale on Press」仅含 `transform: scale(0.97);`，全文检索 100ms 仅出现于无关的 `animation-delay: calc(var(--delay) * 100ms)`）。

## 错误 3: 弹层/下拉展开「快（<300ms）」在 [11] 中不存在

- **文件+行号**: `docs/refer/linear.md` (行 112)
- **原声称**: "| 弹层/下拉展开 | 快（<300ms） | 自定义贝塞尔，**origin-aware**：从触发元素处展开而非居中 | 悬停/点击触发 | [11] |"
- **错误类型**: 来源不支撑
- **正确信息**: [11] 支撑 origin-aware 语义（emil-kowalski.md 第 5 条「Origin-aware animations — Motion should originate from its logical source; a dropdown expands from its trigger, not from center」），但 <300ms 时长数值在 emil-kowalski.md 与 motion-cookbook.md 中均不存在（<300ms 只有「UI animations should generally stay under 300ms」的总体原则，无弹层专项时长）。
- **反证来源**: https://raw.githubusercontent.com/kylezantos/design-motion-principles/main/skills/design-motion-principles/references/emil-kowalski.md（「Keep animations fast — Under 300ms for UI; remove animation entirely for high-frequency interactions」——为总体原则，无弹层专项数值）。

## 错误 4: 四级文字「约 3.6:1 对比度」与计算值不符（约 3.46:1）

- **文件+行号**: `docs/refer/linear.md` (行 37)
- **原声称**: "| 四级 | `#62666D` | 时间戳等最弱文字；对近黑背景约 3.6:1 对比度，低于 WCAG AA（4.5:1），官方用于非正文场景，若作正文则为可访问性缺陷 [6][7] |"
- **错误类型**: 事实错误
- **正确信息**: 按 WCAG 相对亮度公式，#62666D（R98/G102/B109，相对亮度约 0.132）对 #08090A（R8/G9/B10，相对亮度约 0.00269）的对比度为 (0.132+0.05)/(0.00269+0.05) ≈ **3.46:1**，非 3.6:1。「低于 WCAG AA 4.5:1」的结论不受影响（3.46:1 与 3.6:1 均低于 4.5:1）。
- **反证来源**: 计算依据 WCAG 2.x 对比度公式（L1 = 0.2126·R + 0.7152·G + 0.0722·B 线性化后，ratio = (L1+0.05)/(L2+0.05)），色值取自 [7]（#62666D）与 [6]/[7]（#08090A）；可复算验证（近似 3.45–3.46:1）。

## 错误 5: dracula/linear 主题载体描述为「userstyle」，实际为 Linear 官方主题系统 JSON 导入

- **文件+行号**: `docs/refer/linear.md` (行 213、来源表 [16])
- **原声称**: "**dracula/linear**：Dracula 配色的 Linear 暗色主题（userstyle），2025-10 已适配 Linear 新主题系统（官方主题生成器可导入自定义主题）；安装入口 draculatheme.com/linear。"
- **错误类型**: 事实错误（载体类型描述错误；「已适配新主题系统」结论正确）
- **正确信息**: dracula/linear 的官方安装方式（仓库 INSTALL.md 与 draculatheme.com/linear 页面一致）是**复制 JSON 主题**（含 base/accent/contrast + sidebar 结构）→ Settings → Preferences → Import Theme 导入，即 Linear 官方主题系统的 JSON 主题格式，**并非 userstyle**（用户样式扩展）。仓库中亦无 userstyle 文件（仅 README/INSTALL.md/screenshots）。「2025-10 适配」有支撑（仓库 pushed_at 2025-10-31）。
- **反证来源**: https://raw.githubusercontent.com/dracula/linear/master/INSTALL.md（"Copy the JSON theme below : { "base": [...], "accent": [...], "contrast": 30, "sidebar": {...} } … Click `Import Theme`. Paste in the JSON"）；https://draculatheme.com/linear（2026-08-15 抓取，安装说明同为 JSON 导入）；https://api.github.com/repos/dracula/linear（pushed_at 2025-10-31T10:30:39Z）。

## 错误 6: `--speed-*` CSS 变量不存在于 [6] designsystems.one 页面

- **文件+行号**: `docs/refer/linear.md` (行 197、行 125)
- **原声称**: "另有动效速度 CSS 变量记载：`--speed-highlightFadeIn: 0s`、`--speed-highlightFadeOut: .15s`、`--speed-quickTransition: .1s`、`--speed-regularTransition: .25s`，以及 hover 背景色 0.12s、图标箭头 transform 0.15s 的实现值（出处为搜索引擎摘要转述，原始文档未能核实，标注待核）[6]。"
- **错误类型**: 来源不支撑（归因错误）
- **正确信息**: designsystems.one 的 Linear 页面（2026-08-15 抓取全文）只有 motion-fast（180ms cubic-bezier(0.22,1,0.36,1)）、motion-default（250ms cubic-bezier(0.25,0.46,0.45,0.94)）、Fast 100ms ease-out、Default 250ms 四项，**无任何 `--speed-*` 变量**（全文检索 highlightFadeIn/quickTransition/regularTransition 均 0 命中）。文档虽自标「待核」，但将 [6] 挂为出处不准确——该快照不应归因于 [6]。
- **反证来源**: https://www.designsystems.one/design-systems/linear（2026-08-15 抓取；页面 Motion 02 区与 token snapshot 全文无 --speed-* 键名）。

## 错误 7: 「二级表面 #141516 / #191A1B（卡片、下拉等）」中 #141516 角色错配

- **文件+行号**: `docs/refer/linear.md` (行 24)
- **原声称**: "| 二级表面 | `#141516` / `#191A1B` | 卡片、下拉等 [7] |"
- **错误类型**: 事实错误（角色错配）
- **正确信息**: [7] DESIGN.md 中 #191a1b 是「Level 3 Surface: Elevated surface areas, card backgrounds, dropdowns」（卡片/下拉支撑正确）；#141516 的角色是 **「Line Tint: Nearly invisible line for the subtlest divisions」**（最细微分隔线），并非卡片/下拉表面。若需二级表面色，DESIGN.md 的相邻表面为 #16171C（garden-skills）或 #1c1c1f（designsystems.one --linear-surface），均非 #141516。
- **反证来源**: https://raw.githubusercontent.com/sweetkey/awesome-design-md/main/design-md/linear.app/DESIGN.md（「**Line Tint** (`#141516`): Nearly invisible line for the subtlest divisions.」「**Level 3 Surface** (`#191a1b`): Elevated surface areas, card backgrounds, dropdowns.」）。

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：

- **官方博客/Changelog（linear.app 直抓全文）**：「Don't compete for attention you haven't earned」逐字存在（A calmer interface for a product in motion，Charlie Aufmann & Maxime Heckel，2026-03-12）；侧栏调暗「a few notches dimmer」、页签紧凑化「more compact…rounded corners and smaller icon and text sizing…icon-only tabs」、图标治理「reduces icon usage, scales their sizes down, and removes unnecessary visual treatments like colored team icon backgrounds」、边框「rounding out their edges and softening the contrast…fewer separators」、旧配色「cool, blue-ish hue…warmer gray that still feels crisp, but less saturated」、Claude Code dev toolbar 颜色工具「tweaking the hue, chroma, and lightness of individual design tokens…copied the token values as JSON and imported them directly into Figma using a plugin built by…Yann-Edern Gillet」、主题生成器「selecting base UI and accent colors and adjusting contrast」；2024 redesign（2024-03-28）：「using the LCH color space instead of HSL」「instead of having to define 98 specific variables for each theme, we defined three: base color, accent color, and contrast」「We started using Inter Display to add more expression to our headings…kept using regular Inter for the rest」、五里程碑、「documented and defined the behaviors of the main components of the app: sidebar, tabs, app headers, and view headers」、「aligning labels, icons, and buttons, both vertically and horizontally in the sidebar and tabs」、Inbox「more centered around the notification type and emphasized the faces of your teammates. We simplified headers and filters」；Welcome to the new Linear changelog：新侧栏/页签/收件箱、「you can still apply the Magic Blue theme…from the command menu or settings」「new theme generator」、侧栏「less cluttered」（URL slug 2024-03-20 存在）；Contextual command menu changelog（2019）：「The command menu gives you access to all actions applicable to your view or selection」「bringing the command menu closer to the UI element that it was invoked from…acts almost like a drop-down」「Added right click context menu for copying and opening links」（URL slug 2019-10-07 存在）。
- **社区逆向（sweetkey DESIGN.md 抓取全文）**：全部色板（#08090a/#0f1011/#191a1b/#010102/#28282c/#f7f8f8/#d0d6e0/#8a8f98/#62666d/#5e6ad2/#7170ff/#828fff/#7a7fad/#27a644/#10b981/#23252a/#34343a/#3e3e44/rgba(255,255,255,0.05~0.08)/rgba(0,0,0,0.85)）；Inter Variable + cv01/ss03、510/590 权重体系、最大 590 不使用 700、Berkeley Mono 回退栈、-1.584px@72px/-1.056px@48px 负字距、圆角 2/4/6/8/12/22/9999px 阶梯、CTA 8px 16px padding + 6px 圆角、inset rgba(0,0,0,0.2) 0px 0px 12px、焦点环 rgba(0,0,0,0.1) 0px 4px 12px、对话框 5 层阴影堆叠 + Ring、0.02→0.04→0.05 表面亮度阶梯、间距 7px/11px 光学微调。
- **designsystems.one（[6]，2026-08-15 抓取）**：unofficial 标注、--linear-accent #5e6ad2/--linear-text #f7f8f8/--linear-text-secondary #8a8f98/--linear-bg #08090a/--linear-surface #1c1c1f/--linear-border #26262a、motion-fast 180ms cubic-bezier(0.22,1,0.36,1)、motion-default 250ms cubic-bezier(0.25,0.46,0.45,0.94)、Fast 100ms ease-out、120–180ms 视图切换/列表重排/模态进入、字号 title-1 56/61、title-3 24/32、正文 15/24、小字 13/19、micro 12/16、间距 4/8/12/16/24/32、圆角 4/8/12/9999、阴影 0 8px 32px rgba(0,0,0,0.35)、告警 #f2994a、错误 #eb5757。
- **garden-skills linear.md（[10]）**：Ground #08090A、强调色 <5% 像素、hero 渐变 <8% 不透明度、间距 4/8/12/16/24/40/64/96、圆角上限 16px「modest, precise, never gummy」、悬停 ~150ms ease-out、布局 350–450ms quint（cubic-bezier(0.22,1,0.36,1)）。
- **emil-kowalski.md（[11]）**：<300ms 总原则、「A 180ms animation feels more responsive than 400ms」、频率法则（每天数百次不动画/键盘永不动画）、scale(0.97)、不从 scale(0) 动画（scale(0.9) 起）、tooltip 首组延迟后续即时、origin-aware 下拉从触发元素展开、blur(2px) 兜底、clip-path reveal/页签切换（硬件加速/无布局抖动/零额外 DOM）、CSS transition 可中断、键盘路径零动画、Emil 为 Linear 设计工程师。
- **第三方色板站**：[12] colorpickercode linear-dark 含 #080808/#141414（2020 版纯灰记载）；[13] colorpalettegenerator.ai 含 #5E6AD2（18 处）；[14] colorarchive brands/linear 含 5E6AD2（10 处）；#5E6AD2 = RGB(94,106,210) 换算成立。
- **其余**：[8] 少数派（2025-12-11、燕耳Firenze、2019 发布黑底+灰 Inter+淡紫渐变 logo、hero 激光动效、HSL→LCH 与 98→3 变量转述、Inter Display 标题、收件箱 1.5 倍行高）；[9] pustelto（2025-09-07、Header 四段 Title/Tabs/Side/Subheader、ResponsiveSlot + ResizeObserver 优先级隐藏 + MobX store + React context、visibility: hidden 保留 DOM + overflow hidden、+N 按钮激活页签显示标签、dnd-kit 拖拽排序）；[15] 60fps.design 移动端 keyboard/bottom sheet/spring 联动；[16] dracula/linear 仓库存在（pushed 2025-10-31）且新主题系统 JSON 导入格式印证官方 base/accent/contrast 模型；[17] marvkr/linear-shadcn src/app/globals.css 的 HSL token（--background 234 17% 12%、--foreground 0 0% 100%、--card/--popover 236 18% 15%、--primary 235 100% 71%、--border 237 15% 26%）。

## 备注（未列为错误，供参考）

- [4] 与 [5] 的 changelog 页面**渲染日期**（March 27, 2024 / October 8, 2019）与**URL slug 日期**（2024-03-20 / 2019-10-07）不一致，文档标注采用 slug 日期，与所引 URL 一致，未列为错误。
- §3.1「动效只作用于合成属性（transform/opacity，偶用 background-color/border-color），从不触发布局属性（width/height/margin/top/left）[11]」——[11] 仅有 clip-path「no layout shift」等间接支撑，无此完整表述；可能综合自其他 Emil 访谈，来源强度不足（未列为错误，建议阶段 3 酌定）。
- §2.2 回退字体栈与 [6] 一致（'SF Pro Display', -apple-system, sans-serif），[7] 的栈更长，文档取 [6] 口径，可接受。
- §5.1「此前为 HSL，感知不均匀」的转述与官方博客「LCH has the benefit that it's perpetually uniform…We never fully rolled out this system」语境一致（文档省略了"未完全铺开"的过程细节，未列为错误）。
