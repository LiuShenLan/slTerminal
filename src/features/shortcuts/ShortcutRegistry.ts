// ShortcutRegistry.ts — 快捷键注册表单例
//
// 职责：
// - 全局 capture-phase keydown 监听器（唯一，引用计数控制生命周期）
// - 命令注册/注销 + 指纹索引
// - 上下文栈管理（pushContext/popContext 带竞态防护）
// - 匹配算法：IME 守卫 → KeyStroke 严格匹配 → 上下文过滤 → 优先级排序
//
// 设计模式：命令模式（ShortcutCommand）+ 注册表模式（Map）+ 单例模式（模块级实例）

import type { ShortcutCommand, ShortcutContext, ShortcutRegistryAPI, KeyStroke } from "./types";

/** 计算按键指纹，用作索引键（如 "Ctrl+Shift+KeyC"） */
function keystrokeFingerprint(ks: KeyStroke): string {
  const mods: string[] = [];
  if (ks.ctrlKey) mods.push("Ctrl");
  if (ks.shiftKey) mods.push("Shift");
  if (ks.altKey) mods.push("Alt");
  if (ks.metaKey) mods.push("Meta");
  return mods.length > 0 ? `${mods.join("+")}+${ks.code}` : ks.code;
}

/** 从 KeyboardEvent 提取 KeyStroke */
function eventToKeyStroke(e: KeyboardEvent): KeyStroke {
  return {
    ctrlKey: e.ctrlKey,
    shiftKey: e.shiftKey,
    altKey: e.altKey,
    metaKey: e.metaKey,
    code: e.code,
  };
}

/** "global" 上下文常量 */
const GLOBAL = "global";

class ShortcutRegistry implements ShortcutRegistryAPI {
  /** 按 id 索引所有命令，O(1) 注销 */
  private commands = new Map<string, ShortcutCommand>();

  /** 指纹 → 命令列表索引，加速按键匹配 */
  private fingerprintIndex = new Map<string, ShortcutCommand[]>();

  /** 活跃上下文栈（末尾 = 最后聚焦的面板） */
  private contextStack: ShortcutContext[] = [];

  /** 是否已向 window 添加 keydown 监听器 */
  private installed = false;

  /** 已注册命令总数（= commands.size），归零时移除监听器 */
  private refCount = 0;

  // ---- 生命周期 ----

  private ensureListenerInstalled(): void {
    if (!this.installed && this.refCount > 0) {
      this.installed = true;
      window.addEventListener("keydown", this.handleKeyDown, { capture: true });
    }
  }

  private ensureListenerRemoved(): void {
    if (this.installed && this.refCount <= 0) {
      this.installed = false;
      window.removeEventListener("keydown", this.handleKeyDown, { capture: true });
    }
  }

  // ---- 公共 API ----

  /** 注册一组命令，返回注销函数 */
  register(commands: ShortcutCommand[]): () => void {
    const ids: string[] = [];
    for (const cmd of commands) {
      if (!this.commands.has(cmd.id)) {
        this.refCount++;
      }
      this.commands.set(cmd.id, cmd);
      ids.push(cmd.id);
    }
    this.rebuildIndex();
    this.ensureListenerInstalled();

    return () => {
      for (const id of ids) {
        this.unregister(id);
      }
    };
  }

  /** 注销单个命令（按 id），幂等 */
  unregister(id: string): void {
    if (!this.commands.has(id)) return;
    this.commands.delete(id);
    this.refCount--;
    this.rebuildIndex();
    this.ensureListenerRemoved();
  }

  /** 推送上下文到活跃栈（面板 focus 时调用），去重后追加到栈尾 */
  pushContext(context: ShortcutContext): void {
    const idx = this.contextStack.indexOf(context);
    if (idx !== -1) {
      this.contextStack.splice(idx, 1);
    }
    this.contextStack.push(context);
  }

  /**
   * 弹出上下文（面板 blur 时调用）。
   * 仅在栈顶匹配时弹出——防竞态：A blur → B focus 时，A 的 blur 不清 B。
   */
  popContext(context: ShortcutContext): void {
    if (
      this.contextStack.length > 0 &&
      this.contextStack[this.contextStack.length - 1] === context
    ) {
      this.contextStack.pop();
    }
  }

  /** 获取当前栈顶上下文（调试用） */
  getActiveContext(): ShortcutContext | null {
    return this.contextStack.length > 0
      ? this.contextStack[this.contextStack.length - 1]
      : null;
  }

  // ---- 内部方法 ----

  /** 重建指纹索引 */
  private rebuildIndex(): void {
    this.fingerprintIndex.clear();
    for (const cmd of this.commands.values()) {
      const fp = keystrokeFingerprint(cmd.keystroke);
      const list = this.fingerprintIndex.get(fp);
      if (list) {
        list.push(cmd);
      } else {
        this.fingerprintIndex.set(fp, [cmd]);
      }
    }
  }

  /** 全局 keydown 处理器（箭头函数属性，确保 this 绑定） */
  private handleKeyDown = (e: KeyboardEvent): void => {
    // IME 合成中：仅匹配 allowDuringComposition 命令
    const fp = keystrokeFingerprint(eventToKeyStroke(e));
    const candidates = this.fingerprintIndex.get(fp);
    if (!candidates || candidates.length === 0) return;

    // 按上下文 + IME 过滤
    const matching = candidates.filter((cmd) => {
      if (e.isComposing && !cmd.allowDuringComposition) return false;
      return cmd.context === GLOBAL || this.contextStack.includes(cmd.context);
    });

    if (matching.length === 0) return;

    // 排序：priority DESC → contextStack index DESC（栈顶/最后聚焦优先）
    matching.sort((a, b) => {
      if (a.priority !== b.priority) return b.priority - a.priority;
      const aIdx = this.contextStack.lastIndexOf(a.context);
      const bIdx = this.contextStack.lastIndexOf(b.context);
      return bIdx - aIdx;
    });

    const winner = matching[0];
    if (winner.handler(e)) {
      e.preventDefault();
      e.stopPropagation();
    }
  };

  // ---- 测试辅助 ----

  /** 返回已注册命令数量 */
  _commandCount(): number {
    return this.commands.size;
  }

  /** 返回当前上下文栈（副本） */
  _contextStack(): ShortcutContext[] {
    return [...this.contextStack];
  }

  /** 清空所有状态（仅测试用） */
  _reset(): void {
    this.commands.clear();
    this.fingerprintIndex.clear();
    this.contextStack = [];
    this.ensureListenerRemoved();
    this.refCount = 0;
  }
}

/** 模块级单例 */
let instance: ShortcutRegistry | null = null;

/** 获取 ShortcutRegistry 单例 */
export function getShortcutRegistry(): ShortcutRegistry {
  if (!instance) {
    instance = new ShortcutRegistry();
  }
  return instance;
}
