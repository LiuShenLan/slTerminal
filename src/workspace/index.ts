// workspace barrel export——仅导出 Workspace 组件。
// panelRegistry/PANEL_TYPES 等其余 re-export 已清理（FE-35）：全仓零消费
// （grep `from ".../workspace"` 仅 App.tsx 消费 Workspace），
// 需要者直接经 `src/panelRegistry` / `./layoutSerde` 导入。
export { default as Workspace } from "./Workspace";
