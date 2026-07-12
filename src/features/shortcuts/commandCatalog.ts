// commandCatalog.ts — 命令目录（单一真值源）
//
// 所有可重绑命令的静态元数据（title/category/context/defaultKey/priority）集中于此。
// handler 不在此——由各面板工厂（globalCommands / terminal.keyboard / editor.keyboard）
// 在注册时经 commandFromMeta(id, handler) 合并，保证 handler 拿到运行期闭包（如 Terminal 实例）。
//
// 新增可重绑命令 = 在 COMMAND_CATALOG 追加一条 + 在对应工厂提供 handler。

import type { Command, CommandMeta, KeyStroke } from "./types";

/** 简写：构造 KeyStroke */
function key(
  code: string,
  mods: { ctrl?: boolean; shift?: boolean; alt?: boolean; meta?: boolean } = {},
): KeyStroke {
  return {
    ctrlKey: mods.ctrl ?? false,
    shiftKey: mods.shift ?? false,
    altKey: mods.alt ?? false,
    metaKey: mods.meta ?? false,
    code,
  };
}

/** 全部可重绑命令的元数据。id 唯一。默认键对自身 context 必须非保留（有 commandCatalog.test 守卫）。 */
export const COMMAND_CATALOG: readonly CommandMeta[] = [
  {
    id: "global.closeTab",
    title: "关闭当前页签",
    category: "global",
    context: "global",
    defaultKey: key("KeyW", { ctrl: true }),
    priority: 10,
  },
  {
    id: "terminal.copy",
    title: "复制选区",
    category: "terminal",
    context: "terminal",
    defaultKey: key("KeyC", { ctrl: true, shift: true }),
    priority: 100,
  },
  {
    id: "terminal.paste",
    title: "粘贴",
    category: "terminal",
    context: "terminal",
    defaultKey: key("KeyV", { ctrl: true, shift: true }),
    priority: 100,
  },
  {
    id: "terminal.newline",
    title: "插入换行（不提交）",
    category: "terminal",
    context: "terminal",
    defaultKey: key("Enter", { ctrl: true }),
    priority: 100,
  },
  {
    id: "editor.save",
    title: "保存文件",
    category: "editor",
    context: "editor",
    defaultKey: key("KeyS", { ctrl: true }),
    priority: 100,
  },
  {
    id: "editor.toggleWordWrap",
    title: "切换自动换行",
    category: "editor",
    context: "editor",
    defaultKey: key("KeyZ", { alt: true }),
    priority: 100,
  },
];

/** id → 元数据，O(1) 查找 */
export const COMMAND_META_BY_ID: ReadonlyMap<string, CommandMeta> = new Map(
  COMMAND_CATALOG.map((meta) => [meta.id, meta]),
);

/**
 * 由命令 id + handler 合并成运行期 Command。
 * @throws 未知 id 时抛错（防止工厂引用了目录里不存在的命令）
 */
export function commandFromMeta(id: string, handler: Command["handler"]): Command {
  const meta = COMMAND_META_BY_ID.get(id);
  if (!meta) throw new Error(`[shortcuts] 未知命令 id: ${id}`);
  return { ...meta, handler };
}
