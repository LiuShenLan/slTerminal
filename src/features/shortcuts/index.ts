// index.ts — 快捷键模块 barrel export

export type { KeyStroke, ShortcutCommand, ShortcutContext, Priority, ShortcutRegistryAPI } from "./types";
export { getShortcutRegistry } from "./ShortcutRegistry";
export { useShortcutContext } from "./useShortcutContext";
export { createGlobalShortcuts } from "./globalCommands";
