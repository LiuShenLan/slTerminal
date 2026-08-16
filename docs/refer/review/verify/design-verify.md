# design 组逐条核实结果（阶段 3）

> 核实日期: 2026-08-15。判定规则同 terminal-verify.md。

## 验证结果汇总

| 类别 | 数量 |
|------|------|
| Review 正确，已修正源文件 | 20 |
| Review 部分正确，部分修正 | 1 |
| Review 不正确，未修改 | 0 |
| 无法验证 | 0 |
| 总计 | 21 |

## 逐条判定

| 条目 | 判定 | 说明 |
|------|------|------|
| apple 错误 1（38→35） | 接受 | color.json macOS 表 35/35 名称全部存在 |
| apple 错误 2（11→12 个 NSFont API） | 接受 | typography.json 6/6 特征名称存在（12 行表） |
| apple 错误 3（svrnty 页面声明 Apple HIG） | 接受 | 页面 "following Apple HIG" 逐字；「非官方数值」结论不变，支撑句修正 |
| apple 错误 4（colorarchive 无 OKLCH/reduced-motion） | 接受 | 计数均 0 |
| apple 错误 5（Dracula string #F4F99D） | 接受 | 文件值 `0.956863 0.976471 0.615686 1` 确认 |
| apple 错误 6（SO 单答 0.35–0.40s） | 接受 | StackExchange API: 1 条答案、无 0.505、含 0.35 |
| apple 错误 7（photon #139 无关） | 接受 | API: title "Doorhanger component: Understand" |
| apple 错误 8（interiorBackgroundStyle 遗漏） | 接受 | 官方 JSON topicSections 含 interiorBackgroundStyle |
| linear 错误 1（成功色归因） | 接受 | #27a644 在 [7]；[6] 为 #4cb782 |
| linear 错误 2（100ms ease-in 无支撑） | 接受 | emil-kowalski.md 有 scale(0.97)、无 100ms（计数 0） |
| linear 错误 3（弹层 <300ms 无专项支撑） | 接受 | origin-aware 有；<300ms 仅为总原则 |
| linear 错误 4（3.6:1→3.45:1） | **部分接受** | WCAG 公式独立计算 = 3.45:1（review 写 3.46 有 0.01 偏差），修正值取 3.45:1 |
| linear 错误 5（非 userstyle） | 接受 | INSTALL.md "Import Theme" JSON 导入确认 |
| linear 错误 6（--speed-* 不在 [6]） | 接受 | designsystems.one 全文 speed- 计数 0 |
| linear 错误 7（#141516 角色错配） | 接受 | DESIGN.md "#141516 Line Tint" 确认 |
| raycast-arc 错误 1（Mica 2024-03-21） | 接受 | Zendesk API: Mica 文本位置在 March 21 段内（80006 > 77347）、March 14 段无 |
| raycast-arc 错误 2（16→20px 归因 [2]） | 接受 | launch-week-summary 含原文；changelog 1-38-0 "20 pixels" 计数 0 |
| raycast-arc 错误 3（[49] 无"窗口点暗色默认"） | 接受 | 匹配仅为 CSS class（dark:hover:brightness）噪声，无该表述 |
| notion-figma 错误 1（映射表方向误标） | 接受 | global.css 注释为暗色语境替换规则（旧值→新值） |
| notion-figma 错误 2（边框原生值 32,32,32） | 接受 | global.css "rgb(32,32,32) => rgb(37, 37, 37)" 逐字 |
| notion-figma 错误 3（token 示例名虚构） | 接受 | 官方页面 pink/400 + surface/brand-contrast 逐字 |
