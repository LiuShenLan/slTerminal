# 上游基线与重构沿革

> 记录代码无法自证的来源锚点与旧栈删除事实；当前实现细节以代码为准。

## 决策与红线

- **上游基线**：本项目 fork 自上游终端项目（Rust + GPUI 模块化单体终端），baseline commit 为 `e537d528c508e8607d0f5f9fd25e5902f40d661e`（上游 main，2026-10-04）。原因：增量参考需要一个固定比较点。边界：新改动在上游仓库 `git log e537d528..HEAD` 获取后逐个评审回采；不引入 remote/submodule。
- **旧 WebView 双进程栈整体删除**：旧栈的 WebView 前端、IPC、xterm.js、Dockview、CodeMirror、Tauri 命令面与相关预览协议已全部删除，不与新 workspace 并存。原因：单进程 GPUI 定位下的状态双源与跨进程延迟是结构性负担。边界：旧栈实现只从 git 历史追溯，不回流到新代码；旧品牌名不得回流。
- **数据零迁移**：新版只读写 `%APPDATA%\slterm`；旧版便携目录的 `settings.json` 不看不迁。原因：全新 fork，用户数据量小，迁移机制的成本高于重配。边界：旧数据不构成兼容义务。
- **设计输入的事实订正**：上游 baseline 不含套餐余量模块；该能力的真实行为来源是 slTerminal 旧栈 `src-tauri/src/plan_balance/` 终态（旧栈提交 `c59f47f^`）。原因：避免把旧栈能力误记为上游能力。边界：追溯旧行为时以上述提交为准，不以 baseline 缺失为否定依据。
- **版本线**：发布版本续 `0.3.0`；更新链对 `< 0.3.0` 一律视为需手动重装。原因：与既有用户感知连续。边界：版本线、安装器 AppId 与资产命名是发布后不可逆面。

### GPUI 补丁审计

- **固定点**：2026-10-25 完成补丁减法与关窗输入处理器释放修复，Zed 固定为 `d7b616062879de95ecd60dffa05ca6b1d835647a`，gpui-component 固定为 `558ae28d01f871c6b1536f5070704239a9f3a490`。规则：不升级 GPUI 版本线，应用与组件内部的 Zed 依赖必须同 URL、同 rev。原因：避免重复 GPUI 类型，并将 fork 限制在 Windows 产品所需最小补丁集。边界：后续补丁变更必须同提交更新审计表、应用 rev 与锁文件。
- **删减落点**：Zed 在 `fc05d637cc7029d75de051fd7f52c1a0fb8fa6b4` 之后回退 X11 补丁；组件删减提交为 `0ac7d1c8841956def7a69bafe3dcf2dde5457634`，随后同步 Zed rev 与组件锁文件。原因：删除无产品用途的补丁，而不是保留编译关闭的旧能力。边界：不为数学、旧 GFM 或阴影 helper 留兼容入口。

Zed 上游基线为 `eb8e1c8b5502b7007465fbbc465f4a736fa39210`。

| 提交 | 处置 | 规则 / 原因 / 边界 |
| --- | --- | --- |
| `1b913031d01f5b3df8fcab3e8c2ac3568e7f5524` | 保留 | 测试窗口报告不可用的 native handles；虚拟窗口生命周期测试依赖该行为，不影响真实窗口。 |
| `8a1a62727bcb2ead7352e32e7e0d2e919a943535` | 删除 | X11 map events 不属于 Windows 产品面；整补丁回退，不为非 Windows 窗口栈保留定制。 |
| `7c662384cd77c1d1e1566206920e3a420ea6767b` | 保留 | DirectWrite 直接使用 Windows 字体数据所有权，避免额外复制；属于 Windows 字体修复。 |
| `2b0d6d1948e374dc7a1263a27661df1409ae24c1` | 保留 | 彩色 emoji 枚举先推进再读取当前 run；属于 Windows 字体正确性修复。 |
| `fc05d637cc7029d75de051fd7f52c1a0fb8fa6b4` | 保留 | 混合前清理透明彩色 glyph target；属于 Windows GPU 正确性修复。 |
| `d7b616062879de95ecd60dffa05ca6b1d835647a` | 保留 | 关窗时同步释放平台与两帧输入处理器，避免其持有的视图强句柄进入退出泄漏检测；不改变原生窗口延迟销毁所有权。 |

gpui-component 的来源固定提交 `4ee9f274e990d6228e4f276f0a1e48f62f6a2048`、`4eb3917678930561c7ea36742f432313cd30b275` 保留，确保基线和 Zed source identity 明确。

| 提交 | 处置 | 规则 / 原因 / 边界 |
| --- | --- | --- |
| `9a2f663cad2f375e9433a8abdb512009f00d62cc` | 删除 | 应用没有 `set_math_renderer`／`MathSpec` 调用；删除数学桥与专用模块，不留插件注册或占位入口。 |
| `1cacf8b08a87c06f27ecc99748302eb812b5e8c0` | 删除 | 旧 GFM reader 语义补丁捆绑 soft break、HTML 顺序、数学构造与补丁专属 `image_base`；整体退出，不保留过渡字段。 |
| `c20dbef8ed0d4c0fbebbf2e8f02d39e4fa0ac0a7` | 删除 | `popover_shadow` 没有应用调用方；未使用 helper 不占 fork 补丁面。 |
| `fa3763c681dba5b3ea028395148488e1281d478d` | 保留 | 支持 consumer headless windows 的原生 UI 测试；属于测试与窗口生命周期边界。 |
| `05bb03f54d08b80ac059f7daea52707745e05956`、`2dd896281102c7d5a0137ac7c754886dc0d5e1fc`、`e411cc06dfff83a96c4705076483a87a26ea992d` | 保留 | 仅同步测试句柄、窗口生命周期和 DirectWrite 保留补丁对应的 Zed rev；防止组件内部形成第二 GPUI 来源。 |
| `f46ecbb06de5907af9700f3f6e2b5b0845459188` | 保留 | 修复 native text selection release 与 input geometry；属于输入正确性边界。 |
| `f6eb37879723b371e94afd5472ad2459908f7432`、`fc5f5cf63dd80686dafacd2a6e37345bbd1dc7ba` | 保留 | 仅同步彩色 emoji 与透明 glyph 保留补丁对应的 Zed rev；不引入额外产品语义。 |
| `558ae28d01f871c6b1536f5070704239a9f3a490` | 保留 | 仅同步关窗输入处理器释放修复对应的 Zed rev 与组件锁文件；保持单一 GPUI 来源。 |

## 验收与测试

- 上游基线 commit 与旧栈来源提交作为证据锚点，不进入自动化断言。
- GPUI 固定 rev、锁文件单一来源与补丁审计锚由 `GpuiPinTests` 机械核验。
- 2026-10-25 真机验收（Windows 11 10.0.26300，上述固定 rev 的 debug 产物）：真实 GPUI 窗口非空上屏；首条 `Write-Output GPUI_FINAL_SMOKE_OK` 无 DA1 污染、首字符完整且输出正确；正常关窗后进程退出码为 0，未出现 `TerminalView` 实体泄漏。
- 旧栈删除的完整性由架构门禁与禁名门禁兜底：workspace 成员与依赖策略全等，旧品牌名不得出现在新增内容。

## 操作

- 回采上游改动：在上游仓库执行 `git log e537d528..HEAD`，逐条评审后移植；不新增 remote。
- 发布版本号与打包规则见 `.claude/docs/release/packaging.md`。
