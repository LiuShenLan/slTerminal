# 窗口视效 / 壁纸

## 决策与红线

- **双通道互斥**：Mica/MicaAlt 走 DWM `DWMWA_SYSTEMBACKDROP_TYPE`（22621+），
  Aero/Acrylic 走 `SetWindowCompositionAttribute`；1809(17763)+ 的 Mica 回退
  `Blurred` AccentPolicy，更老系统纯透明。每档都把另一通道写回中性值——双通道
  同开时 DWM 行为未定义（实测 backdrop 吞 Acrylic）。gate/动作计划是纯函数
  `background_appearance_for`/`material_plan`，真实 Win32 调用只消费计划。
- **`None` 也保留透明交换链**：GPUI Windows 渲染器按背景外观选清屏 alpha，
  `Opaque` 固定 alpha=1，会盖死窗口透明度；透明交换链 + WCA 全零 = 无材质。
- **透明度纪律**：`opacity` 只乘壳底色 token（background/sidebar/title_bar 及
  分隔线）；终端默认格透明，由同一壳底色层透出（当前无独立终端卡底色，
  避免同区域二次 alpha），文字前景恒不透明。重算前把基线 alpha 归一到 1，
  滑块拖动/异步壁纸就绪可高频重复调用而不层层相乘。
- **壁纸有界加载**：文件 64 MiB、解码 128 MiB、纹理 8 MiB、长边 2048
  （预览 512 边）为契约值；单 job 串行，UI 线程零文件 I/O。代际失配
  （`generation != version`）整帧丢弃，不做协作式中断；文件戳（mtime+字节）
  不变即复用纹理。解码失败保留旧纹理/空壁纸，只记日志。
- **图层语义**：壁纸在底色之上、单元格之下；`cover_chrome` 决定整窗层
  （`window_layer`）或终端内容区卡层（`card_layer`），两层互斥绘制。
  `background_image_opacity` 独立于窗口 opacity。
- **静默启动缝**：本模块任何路径都不得 show/activate/聚焦窗口；`refresh`
  只改背景外观、启动后台解码。首窗 `show=false`/`focus=false` 由开窗链独占
  （门禁用例 `visual_effects_never_show_or_activate_windows`）。
- **消费点**：`Settings::{blur,opacity,background_image*}`（值权威在
  `slterm-settings`）；`theme::apply_dark_theme`（启动）、设置页 `Changed`
  （热应用）、`SettingsPane::preview_opacity`（live 快览）、`open_initial_window`
  （首窗 backdrop + 首载）。`load_preview` 的消费点 = 设置页壁纸预览
  （`SettingsPane` 外观页，后台解码 + 前台缩略图）；测试豁免登记见
  `.claude/test-exemptions.md` M10.7 行。
