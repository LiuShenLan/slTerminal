# slterm_split（core 层）

> 分屏规则的共享权威：布局树、几何切割、分隔条拖拽比例与方向导航的纯数据/纯函数实现。

## 决策与红线

- **零生产依赖**：不得新增生产依赖；不得出现任何 UI 框架类型（GPUI/窗口/实体）。
- **仅网格分屏**：分屏只允许网格切割，禁止 floating/popout。原因：非网格组的归属推导、持久化与恢复链会整体复杂化，且无产品需求。边界：壳不得调用浮动/弹出 API。
- **坐标系合同**：屏幕像素、左上原点；`Rect` 是 pane 的真实分配矩形。壳负责把窗口尺寸与 padding 换算成 pane 矩形，换算属壳内细节，不进本 crate。
- **契约常量**（`DIVIDER_GAP`/`HIT_SLOP`/`CLOSE_MARGIN`/`RATIO_CLAMP`）是行为合同；改动须同步壳层与 UI 用例。
- **身份类型**：`PaneId`/`TabId` 是跨领域锚点；他处只许 `use`，禁重定义或裸 `u64` 漂移。
- **删除/焦点语义**：关 pane 的结局由 `RemoveOutcome` 裁定（关整 tab / 兄弟收编），壳不得自行散判。原因：删除语义分散会产生孤儿 pane 与焦点错位。边界：新增删除路径必须经同一裁定。
- **布局单点**：会话布局只经 session schema 的 layout 转换对存取；本 crate 只提供纯函数，不持序列化通道。

## 验收与测试

- 定向测试：`cargo test -p slterm-split --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- 切割数学、比例曲线、最近邻导航、树变换与删除结果是必测面；纯函数用例不得依赖窗口。
- 测试分层与隔离规则见 `.claude/docs/testing/strategy.md`。

## 操作

- 新增分屏能力：先在本 crate 以纯函数/纯数据表达，再由壳消费结果；不得在壳内重算分屏语义。
- 修改契约常量时同步壳层阈值与 UI 拖拽用例。
