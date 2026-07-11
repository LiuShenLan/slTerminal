// types.ts — 快捷键模块核心类型定义
//
// 设计：Command / Keybinding 分离（决策 B1）
//   - CommandMeta：静态元数据（id/title/category/context/defaultKey/priority），代码定义、可枚举
//   - Command = CommandMeta + handler（高内聚：动作行为随命令）
//   - 运行期"键位 → 命令"由 ShortcutRegistry 按 默认键 ⊕ 用户覆盖层 合并得到（低耦合：键位可换）
//   - KeybindingOverrides：用户覆盖层（commandId → keystroke 字符串 | null 解绑）

/** 按键组合（所有字段 required，匹配时严格相等） */
export interface KeyStroke {
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  /** KeyboardEvent.code，如 "KeyC"、"KeyV" */
  code: string;
}

/** 上下文标识符，对应面板类型。"global" 表示始终活跃 */
export type ShortcutContext = string;

/**
 * 优先级：数字越大越优先。
 * 0-99 全局/fallback，100-199 面板级（terminal/editor），200+ 覆盖级（调试/测试）
 */
export type Priority = number;

/** 命令分类（用于未来可视化设置 UI 分组） */
export type CommandCategory = "global" | "terminal" | "editor";

/**
 * 命令静态元数据（不含 handler，可在面板未挂载时枚举）。
 * 是 defaultKey / title / priority 的单一来源（见 commandCatalog.ts）。
 */
export interface CommandMeta {
  /** 全局唯一标识，如 "terminal.copy"、"editor.save" */
  id: string;
  /** 展示名（未来 UI 用） */
  title: string;
  /** 分类（未来 UI 分组用） */
  category: CommandCategory;
  /** 所属上下文 */
  context: ShortcutContext;
  /** 默认按键组合；null 表示命令存在但无默认键 */
  defaultKey: KeyStroke | null;
  /** 匹配优先级 */
  priority: Priority;
  /** 若为 true，即使在 IME 组合态中也触发（默认 false = IME 透传） */
  allowDuringComposition?: boolean;
}

/**
 * 运行期命令 = 元数据 + 执行函数。
 * handler 返回 true → 调用方 preventDefault()+stopPropagation()（阻止浏览器/xterm.js 内置行为）
 * handler 返回 false → 透传（事件继续冒泡）
 */
export interface Command extends CommandMeta {
  handler: (event: KeyboardEvent) => boolean;
}

/**
 * 用户覆盖层：commandId → keystroke 字符串（如 "Ctrl+Alt+KeyC"）或 null（显式解绑）。
 * 缺省的 command 使用其 defaultKey。持久化于 ~/.slterminal/settings.json 的 keybindings 段。
 */
export type KeybindingOverrides = Record<string, string | null>;

/** 单条绑定导出项（exportContextBindings 返回，供未来 iframe 转发脚本用） */
export interface ExportedBinding {
  id: string;
  keystroke: string;
}

/** ShortcutRegistry 公共接口 */
export interface ShortcutRegistryAPI {
  /** 注册一组命令，返回注销函数 */
  register(commands: Command[]): () => void;
  /** 注销单个命令（按 id） */
  unregister(id: string): void;
  /** 推送上下文到活跃栈（面板 focus 时调用） */
  pushContext(context: ShortcutContext): void;
  /** 从活跃栈弹出上下文（面板 blur 时调用，带身份校验防竞态） */
  popContext(context: ShortcutContext): void;
  /** 获取当前栈顶上下文（调试用） */
  getActiveContext(): ShortcutContext | null;
  /** 设置用户覆盖层并重建绑定表 */
  setOverrides(overrides: KeybindingOverrides): void;
  /**
   * 直接解析并执行一个键盘事件（供 xterm attachCustomKeyEventHandler 委托）。
   * forceContext 指定时强制在该 context 下匹配（不依赖 contextStack）。
   * 返回是否被命令消费（不调用 preventDefault——无 window 事件可取消）。
   */
  resolve(event: KeyboardEvent, forceContext?: ShortcutContext): boolean;
  /** 导出某 context 当前生效的绑定（含 global，排除解绑），供未来 iframe 转发用 */
  exportContextBindings(context: ShortcutContext): ExportedBinding[];
  /** 列出当前已注册命令的元数据（未来 UI 用） */
  listCommands(): CommandMeta[];
  /** 返回已注册命令数量（测试/调试用） */
  _commandCount(): number;
  /** 返回当前上下文栈（测试/调试用） */
  _contextStack(): ShortcutContext[];
  /** 清空所有注册和上下文（仅测试用） */
  _reset(): void;
}
