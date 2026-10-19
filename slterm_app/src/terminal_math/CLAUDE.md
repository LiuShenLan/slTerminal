# 终端数学覆盖层

> 在终端绘制阶段标注可见公式源格；不改变终端状态与复制文本。

## 决策与红线

- **终端仍是真值**：覆盖层只替换 paint pass 中的可见定界符跨度；不得修改 cell、滚动、光标、选区或复制文本。选区把公式跨度当原子，映射源边界。
- **扫描必须有界**：失败候选也计成本。可见公式 64、持久公式 2048、持久预算 1 MiB、历史回看 512 行、裸括号 8 行 / 裸方括号 24 行、整帧裸搜索 4k cell、空行借用 2 行、高度溢出容差 0.12——全部不得放宽。原因：不可信终端输出不得让标注退化为全帧扫描。边界：新增扫描路径必须有 O(1) 意图预过滤或共享帧预算。
- **同裁定**：`LineProjection` 必须与终端 reflow/选区采用同一裁决；位图、`CoverageMask` 与后续文字不得落在三套坐标。调用方对齐存活表。
- **覆盖语义**：`CoverageMask` 只遮会渲染的公式源格；布局失败/不可读即保留原文，不遮不画。
- **后端无关**：本模块不依赖 GPUI；GPUI paint pass 与快照映射归消费方。

## 验收与测试

- 定向测试：`cargo test -p slterm --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 必测面：有界扫描预算、`LineProjection` 与终端 reflow/选区同裁定、覆盖遮罩只遮源格、失败保留原文。
- 上屏后的字形观感由构建产物人工目测，边界见 `.claude/docs/testing/exemptions.md`。

## 操作

- 绘制数据面：在终端快照同一锁窗口构造 `TextGrid` 并调用 `scan_visible`；`MathViewport` 从元素布局构造，`OverlayCell` 从同一锁窗口的网格 cell 映射（调色板解析前景 + 背景存在性 + 从 cell `Flags` 取 DIM）。
- `prepare_overlays` → `plan_overlay_draw` / `CoverageMask` 是绘制数据面；`LayoutResolver` 注入引擎编译句柄。
- 调整扫描上限：属契约数值变更，须评审并补边界用例。
