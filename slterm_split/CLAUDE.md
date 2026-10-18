# slterm_split（core 层）

## 存在理由与边界

分屏规则的共享权威：布局树、几何切割、分隔条拖拽比例与方向导航的纯数据/纯函数实现。壳只消费结果，不重算分屏语义。

- **零生产依赖契约**：不得新增生产依赖；不得出现任何 UI 框架类型（GPUI/窗口/实体）。
- **坐标系合同**：屏幕像素、左上原点；`Rect` 是 pane 的真实分配矩形。壳负责把窗口尺寸与 padding 换算成 pane 矩形，换算属壳内细节，不进本 crate。
- **契约常量**（`DIVIDER_GAP`/`HIT_SLOP`/`CLOSE_MARGIN`/`RATIO_CLAMP`）是行为合同；改动须同步壳层与 UI 用例。
- **身份类型**：`PaneId`/`TabId` 是跨领域锚点；他处只许 `use`，禁重定义或裸 `u64` 漂移。
- **删除/焦点语义**：关 pane 的结局由 `RemoveOutcome` 裁定（关整 tab / 兄弟收编），壳不得自行散判。

## 测试

- 定向测试：`cargo test -p slterm-split --test lib_tests <filter> -- --test-threads=1`；禁 `--lib`。
- 切割数学、比例曲线、最近邻导航、树变换与删除结果是必测面；纯函数用例不得依赖窗口。
