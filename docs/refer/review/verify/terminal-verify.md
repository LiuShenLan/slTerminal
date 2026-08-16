# terminal 组逐条核实结果（阶段 3）

> 核实日期: 2026-08-15。每条 review 错误条目经独立复核（curl 直抓反证来源逐字确认），判定：接受（修改源文件）/ 拒绝（review 有误）/ 部分接受。

## 验证结果汇总

| 类别 | 数量 |
|------|------|
| Review 正确，已修正源文件 | 11 |
| Review 部分正确，部分修正 | 0 |
| Review 不正确，未修改 | 1 |
| 无法验证 | 0（lushbinary 内容无法抓取，但 120fps 无支撑由 [20][21] 双源证实） |
| 总计 | 12 |

## 逐条判定

| 条目 | 判定 | 说明 |
|------|------|------|
| windows-terminal 错误 1（#8405 disablePaneAnimations 不存在） | **拒绝** | 独立复核：该字样存在于 zadjii-msft 评论 "Something like `disablePaneAnimations` would I guess make more sense"——原文档声称成立，review 反证有误，源文件不改 |
| windows-terminal 错误 2（QuadraticEase） | 接受 | PR #7364.diff 两处 `animation.EasingFunction(Media::Animation::QuadraticEase{})` 确认 |
| windows-terminal 错误 3（#14858 已关闭） | 接受 | API: state closed / closed_at 2023-07-05 / milestone v1.18 |
| windows-terminal 错误 4（#B3FFFFFF 是前景色） | 接受 | CommonResources.xaml: `DeleteButtonForegroundPressed Color="#B3FFFFFF"`；BackgroundPressed 为 Opacity 0.8 + DeleteButtonColor |
| warp 错误 1（Hack 官方声明） | 接受 | 官方页 "Warp's default font, Hack, doesn't yet have ligature support" 逐字 |
| warp 错误 2（内置主题部分开源） | 接受 | warp_bundled/ 目录含 dracula.yaml 等 13 个 |
| warp 错误 3（120fps 无支撑） | 接受 | [20] 无 120fps、[21] 新闻稿 "120" 仅为 og:image 1200 等元数据；[24] lushbinary 页面无法抓取（JS），但双源已证无支撑 |
| warp 错误 4（137→130） | 接受 | git trees API: standard/*.yaml = 130 |
| warp 错误 5（dashed 非 wavy） | 接受 | 官方原文 "a dashed red underline" 逐字 |
| terminals-other 错误 1（PR#627 合并日期） | 接受 | API: closed_at 2026-04-09T00:15:35Z |
| terminals-other 错误 2（Tabby 平台字体） | 接受 | config.ts: Windows Consolas / Linux Liberation Mono / macOS Menlo 逐字 |
| terminals-other 错误 3（168→170） | 接受 | README images/*.png 唯一计数 = 170 |
