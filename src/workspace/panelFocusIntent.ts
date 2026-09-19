// panelFocusIntent — 面板键盘聚焦意图令牌（C4「新建即聚焦」协作件）
//
// 语义：打开入口在 addPanel **之前**写入意图（mark），面板挂载后由
// usePanelActivationFocus 消费（consume，take 语义——消费即删，一次性）。
// 意图仅覆盖「挂载即激活」路径：面板挂载时若已激活（dockview addPanel 默认
// 激活新面板），挂载期 effect 双条件（isActive && isGroupActive）已满足，
// 不会再有激活事件触发——意图令牌是这条路径的唯一焦点驱动。
//
// 反向路径零依赖：布局批量恢复（fromJSON）从不写意图 → 恢复的面板即使
// 挂载即激活也不抢焦（豁免语义天然成立，无时序依赖）；页签点击/去重命中
// focus()/switchToPageAndFocus 走激活事件路径（不需意图）。
//
// 滞留边界（登记不防）：mark 后 addPanel 抛错 → 意图残留；panelId 含唯一
// 后缀（时间戳/计数），残留意图永不被误消费，模块级 Set 有界。

/** 在途聚焦意图集合（panelId 页前缀协议 id） */
const intents = new Set<string>();

/** 写入聚焦意图（addPanel 前调用；重复 mark 幂等） */
export function markPanelFocusIntent(panelId: string): void {
  intents.add(panelId);
}

/** 消费聚焦意图（take 语义——存在即删并返回 true，否则 false） */
export function consumePanelFocusIntent(panelId: string): boolean {
  return intents.delete(panelId);
}

/** 测试专用：清空全部在途意图（beforeEach/afterEach 用例隔离） */
export function _resetPanelFocusIntent(): void {
  intents.clear();
}
