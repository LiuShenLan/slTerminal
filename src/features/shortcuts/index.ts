// index.ts — 快捷键模块 barrel export

export type {
  KeyStroke,
  Command,
  CommandMeta,
  CommandCategory,
  ShortcutContext,
  Priority,
  KeybindingOverrides,
  ExportedBinding,
  ShortcutRegistryAPI,
} from "./types";
export { getShortcutRegistry } from "./ShortcutRegistry";
export { usePanelFocus } from "./usePanelFocus";
export { createGlobalShortcuts } from "./globalCommands";
export { COMMAND_CATALOG, COMMAND_META_BY_ID, commandFromMeta } from "./commandCatalog";
export { formatKeystroke, parseKeystroke, isValidKeystrokeString } from "./keystroke";
export { isReserved } from "./reserved";
export { wireKeybindings } from "./wireKeybindings";
