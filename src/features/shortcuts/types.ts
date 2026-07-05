// types.ts — 快捷键模块核心类型定义
//
// KeyStroke：描述按键组合（所有修饰键字段为 required boolean，匹配时严格相等比较）
// ShortcutCommand：单个快捷键命令（命令模式：handler 返回 true 阻止事件，false 透传）
// ShortcutRegistryAPI：注册表公共接口

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

/** 单个快捷键命令 */
export interface ShortcutCommand {
  /** 全局唯一标识，如 "terminal.copy"、"terminal.paste" */
  id: string;
  /** 按键组合 */
  keystroke: KeyStroke;
  /** 所属上下文 */
  context: ShortcutContext;
  /** 匹配优先级 */
  priority: Priority;
  /** 若为 true，即使在 IME 组合态中也触发（默认 false = IME 透传） */
  allowDuringComposition?: boolean;
  /**
   * 执行函数。
   * 返回 true → 调用方执行 preventDefault() + stopPropagation()（阻止浏览器/xterm.js 内置行为）
   * 返回 false → 透传（事件继续冒泡）
   */
  handler: (event: KeyboardEvent) => boolean;
}

/** ShortcutRegistry 公共接口 */
export interface ShortcutRegistryAPI {
  /** 注册一组命令，返回注销函数 */
  register(commands: ShortcutCommand[]): () => void;
  /** 注销单个命令（按 id） */
  unregister(id: string): void;
  /** 推送上下文到活跃栈（面板 focus 时调用） */
  pushContext(context: ShortcutContext): void;
  /** 从活跃栈弹出上下文（面板 blur 时调用，带身份校验防竞态） */
  popContext(context: ShortcutContext): void;
  /** 获取当前栈顶上下文（调试用） */
  getActiveContext(): ShortcutContext | null;
  /** 返回已注册命令数量（测试/调试用） */
  _commandCount(): number;
  /** 返回当前上下文栈（测试/调试用） */
  _contextStack(): ShortcutContext[];
  /** 清空所有注册和上下文（仅测试用） */
  _reset(): void;
}
