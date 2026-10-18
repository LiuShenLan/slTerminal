# 终端数学覆盖层

## 决策与红线

- **终端仍是真值**:覆盖层只替换 paint pass 中的可见定界符跨度;不得修改
  cell、滚动、光标、选区或复制文本。选区把公式跨度当原子,映射源边界。
- **扫描必须有界**:失败候选也计成本。可见公式 64、持久公式 2048、
  持久预算 1MB、历史回看 512 行、裸括号 8 行 / 裸方括号 24 行、整帧裸
  搜索 4k cell、空行借用 2 行、高度溢出容差 0.12——全部不得放宽。
  新增扫描路径必须有 O(1) 意图预过滤或共享帧预算。
- **同裁定**:`LineProjection` 必须与终端 reflow/选区采用同一裁决;位图、
  `CoverageMask` 与后续文字不得落在三套坐标。调用方对齐存活表。
- **覆盖语义**:`CoverageMask` 只遮会渲染的公式源格;布局失败/不可读即
  保留原文,不遮不画。
- **后端无关**:本模块不依赖 GPUI;GPUI paint pass 与快照映射归 M9.5。

## 步骤接缝

- M9.5 在终端快照同一锁窗口构造 `TextGrid` 并调用 `scan_visible`;
  `MathViewport` 从元素布局构造,`OverlayCell` 从同一锁窗口的网格 cell
  映射(调色板解析前景 + 背景存在性 + 从 cell `Flags` 取 DIM——
  `RenderSnapshot::SnapCell` 不含 DIM)。
- `prepare_overlays` → `plan_overlay_draw` / `CoverageMask` 是绘制数据面;
  `LayoutResolver` 注入引擎编译句柄。
