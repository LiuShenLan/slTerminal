# devtools 组逐条核实结果（阶段 3）

> 核实日期: 2026-08-15。判定规则同 terminal-verify.md。

## 验证结果汇总

| 类别 | 数量 |
|------|------|
| Review 正确，已修正源文件 | 13 |
| Review 部分正确，部分修正 | 1 |
| Review 不正确，未修改 | 0 |
| 无法验证 | 1 |
| 总计 | 14（+1 无法验证项不计入错误条目） |

## 逐条判定

| 条目 | 判定 | 说明 |
|------|------|------|
| jetbrains 错误 1（Table 无 rowHeight.compact） | 接受 | darcula.theme.json Table 节点键集无 rowHeight.compact，rowHeight=20 |
| vscode 错误 1（右键菜单色） | 接受 | dark_vs.json: menu.background #252526 / selection #0078d4 / separator #454545 逐字；与 2.1.3 表矛盾一并消除 |
| zed 错误 1（selector 色） | 接受 | one.json: selector #dfc184ff，number/boolean #bf956aff |
| zed 错误 2（13→14 组） | 接受 | 14 个语义状态色键全部存在、0 缺失 |
| zed 错误 3（cmd-shift-p） | 接受 | 官方文档 "cmd-shift-p\|ctrl-shift-p" 逐字 |
| zed 错误 4（141 顶层键） | 接受 | one.json style 顶层键数实测 141 |
| zed 错误 5（reduce_motion 在 animation.rs） | 接受 | animation.rs 含 reduce_motion；editor.rs 计数 0 |
| zed 错误 6（PR#40035 未合并） | 接受 | API: merged false |
| zed 错误 7（PR#8241 未合并 + 设置名过时） | 接受 | API: merged false；现行设置名按 all-settings 文档 |
| devtools-other 错误 1（ST3.2 版本归属） | **部分接受** | ST3.2 博客无 Adaptive（计数 0）属实；取色机制仅 [8] 部分支撑——修正为去掉版本归属、机制保留 [8] |
| devtools-other 错误 2（ST4 Mariana 首推归因） | 接受 | ST4 博客 "Changed default color scheme to Mariana" 逐字确认（[4] 为正源） |
| devtools-other 错误 3（v0.15 版本归属） | 接受 | 论坛帖全文无 "0.15"（计数 0） |
| devtools-other 错误 4（10pt 引用不支撑） | 接受 | themes.html 无默认字号表述；10pt 事实保留、出处标注调整 |
| devtools-other 错误 5（8→6 档） | 接受 | Tabs.md --tab-text-color* 唯一变量计数 = 6 |

## 无法验证项

- vscode "1.91 起默认 Dark Modern" 版本归属：来源无版本号、release notes 无记载、dark_modern.json 最早提交 2023-05——按"默认不修改"原则源文件不改（review 阶段未列为错误条目）。
