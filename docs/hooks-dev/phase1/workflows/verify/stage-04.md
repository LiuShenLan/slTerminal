# Stage 04 Verify：页签四态集成

> 断言与 Stage 04 完成后的真实中间态一致。

## useCommandDetection

- [ ] `P1-F3-01` `src/panels/terminal/useCommandDetection.ts` 中 OSC 133 C 匹配规则时调用 `onTabStateChange({ active: true, title: rule.title, icon: "🟡" })`。
- [ ] `P1-F3-01` OSC 133 D 时调用 `onTabStateChange({ active: false })`。
- [ ] `P1-F3-01` 标题切换逻辑保留。

## useXterm

- [ ] `P1-F3-02` `src/panels/terminal/useXterm.ts` 从 `../../ipc` import `hooks`。
- [ ] `P1-F3-02` `src/panels/terminal/useXterm.ts` 从 `../../lib/claudeStatus` import `eventToStatus`、`STATUS_EMOJI`。
- [ ] `P1-F3-02` `useEffect` 中订阅 `onHookEvent`。
- [ ] `P1-F3-02` 过滤条件为 `payload.panelId === panelId`。
- [ ] `P1-F3-02` `SessionEnd` 事件触发 `onTabStateChange({ active: false })`。
- [ ] `P1-F3-02` 非 null 状态触发 `onTabStateChange({ active: true, icon: STATUS_EMOJI[status] })`。
- [ ] `P1-F3-02` 组件卸载时 unsubscribe。

## TerminalPanel

- [ ] `P1-F3-03` `src/panels/terminal/TerminalPanel.tsx` 的 `handleTabStateChange` 在 `state.active` 时：仅 `state.title` 存在才 `setTitle`，仅 `state.icon !== undefined` 才 `updateParameters`。
- [ ] `P1-F3-03` `state.active` 为 false 时恢复原标题并 `updateParameters({ ...params, tabIcon: null })`。
- [ ] `P1-F3-03` `originalTitleRef` 不在 `state.active` 分支内被覆盖。

## DefaultTab 与 tabRules

- [ ] `P1-F3-04` `src/workspace/PageDockviewHost.tsx` 的 `DefaultTab` 对非 URL/路径型 `tabIcon` 渲染 `<span>`。
- [ ] `P1-F3-04` URL/路径型 `tabIcon` 仍渲染 `<img>`。
- [ ] `P1-F3-05` `src/panels/terminal/tabRules.ts` 不再 import `claudeLogo`。
- [ ] `P1-F3-05` `tabTitleRegistry.register({ command: "claude", title: "claude" })` 无 `icon` 字段。

## 测试

- [ ] `P1-F3-07` useXterm 相关测试覆盖 hook-event panelId 过滤、状态映射、SessionEnd 清 icon。
- [ ] `P1-F3-07` DefaultTab 测试覆盖 emoji 与图片分支。
- [ ] `P1-F3-07` tabRules 测试验证无 icon。
- [ ] `npm test` 全量通过。

## 静态检查

- [ ] `npx tsc --noEmit` 通过。
- [ ] `npx eslint src/panels/terminal src/workspace/PageDockviewHost.tsx` 通过。
