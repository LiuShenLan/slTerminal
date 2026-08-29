// dirtyRegistry.ts —— 设置中心 dirty 汇聚真值源（F11，SC-FE-07）
//
// 壳（SettingsPanel）与 DefaultTab × 关闭守卫共享同一 dirty 真值源，防两处各自维护
// 状态漂移：壳挂载注册（false）、卸载 clear；页 dirty 变化同步 set；× 关闭拦截经
// isSettingsDirty(panel.id) 决定是否弹确认（dirty 只在壳实例存活期间有意义——面板
// 关闭/卸载即清除，重启恢复不可能 dirty）。
// 模块级 Map 单例（硬约束 #13 注册表家族契约形态；测试隔离经 clearSettingsDirty 清理）。

/** panelId → dirty 映射（真值源；无条目 = 非 dirty） */
const dirtyByPanelId = new Map<string, boolean>();

/** 设置面板 dirty 上报（壳挂载注册 / 页 dirty 变化 / 确认丢弃清除 时调用） */
export function setSettingsDirty(panelId: string, dirty: boolean): void {
  dirtyByPanelId.set(panelId, dirty);
}

/** 查询面板 dirty（DefaultTab × 关闭守卫消费；无条目视作非 dirty） */
export function isSettingsDirty(panelId: string): boolean {
  return dirtyByPanelId.get(panelId) === true;
}

/** 清理面板 dirty 条目（壳卸载调用——防条目泄漏） */
export function clearSettingsDirty(panelId: string): void {
  dirtyByPanelId.delete(panelId);
}
