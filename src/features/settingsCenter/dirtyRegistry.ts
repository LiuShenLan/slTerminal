// dirtyRegistry.ts —— 设置中心 dirty 汇聚真值源（F11，SC-FE-07）
//
// 壳（SettingsPanel）与关闭守卫共享同一 dirty 真值源。CP-017 后条目生命周期脱离壳：
// 不写挂载注册、不做卸载 clear；条目在「确认丢弃关闭」动作点清除（tabClose.ts /
// SC-FE-08 守卫）。无条目 = 非 dirty。
// 模块级 Map 单例（硬约束 #13 注册表家族契约形态；测试隔离经 clearSettingsDirty 清理）。

/** panelId → dirty 映射（真值源；无条目 = 非 dirty） */
const dirtyByPanelId = new Map<string, boolean>();

/** 设置面板 dirty 上报（页 dirty 变化 / 确认丢弃后复位 false 时调用） */
export function setSettingsDirty(panelId: string, dirty: boolean): void {
  dirtyByPanelId.set(panelId, dirty);
}

/** 查询面板 dirty（tabClose.ts 单/批量关闭守卫与壳内 SC-FE-08 守卫消费；无条目视作非 dirty） */
export function isSettingsDirty(panelId: string): boolean {
  return dirtyByPanelId.get(panelId) === true;
}

/** 清理面板 dirty 条目（「确认丢弃关闭」动作点调用——CP-017 条目唯一清除通道） */
export function clearSettingsDirty(panelId: string): void {
  dirtyByPanelId.delete(panelId);
}
