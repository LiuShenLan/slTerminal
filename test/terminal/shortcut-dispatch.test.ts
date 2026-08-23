// L3 — 生产按键分发链路（TQ-E-02）：node 环境直接挂生产 keyEventHandler.ts 的
// handleTerminalKeyEvent，验证 Ctrl 组合键 / Shift+Tab 的分发行为：
//   - 已注册命令（terminal.copy = Ctrl+Shift+C）→ 返回 false + preventDefault（被注册表消费）
//   - 未注册键（无修饰）→ 返回 true（透传给 xterm.js）
//   - 非 keydown（keyup）→ 返回 true（透传）
//   - Shift+Tab：terminal context 无绑定且为保留键（isReserved，不可绑）→ 透传 true
//     （xterm.js 反缩进，Shift+Tab 由 attachCustomKeyEventHandler 之外的层接管）
// ShortcutRegistry 用真实注册表（模块级单例 getShortcutRegistry）+ _reset 隔离。
// L3 node 环境无 window/KeyboardEvent——vi.stubGlobal 提供最小 stub：
//   ① window：注册命令触发 ensureListenerInstalled 的 window.addEventListener（仅记录）
//   ② KeyboardEvent：resolve 只需读 type/code/修饰键/isComposing，preventDefault 记录调用
// 用例中构造的事件仅用于解析，不派发到任何 DOM。

import { describe, it, expect, vi, beforeAll, afterAll, beforeEach, afterEach } from 'vitest';
import { handleTerminalKeyEvent } from '../../src/panels/terminal/keyEventHandler';
import { getShortcutRegistry, commandFromMeta } from '../../src/features/shortcuts';

/** L3 node 环境最小 KeyboardEvent stub（覆盖 resolve 读取的全部字段 + preventDefault 记录） */
class KeyEventStub {
  readonly type: string;
  readonly code: string;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly metaKey: boolean;
  readonly isComposing: boolean;
  defaultPrevented = false;

  constructor(init: {
    type?: string;
    code: string;
    ctrlKey?: boolean;
    shiftKey?: boolean;
    altKey?: boolean;
    metaKey?: boolean;
    isComposing?: boolean;
  }) {
    this.type = init.type ?? 'keydown';
    this.code = init.code;
    this.ctrlKey = init.ctrlKey ?? false;
    this.shiftKey = init.shiftKey ?? false;
    this.altKey = init.altKey ?? false;
    this.metaKey = init.metaKey ?? false;
    this.isComposing = init.isComposing ?? false;
  }

  preventDefault(): void {
    this.defaultPrevented = true;
  }
}

/** 构造 keydown 事件（stub 覆盖生产读取的全部字段，类型断言到 KeyboardEvent） */
function keyDown(opts: { code: string; ctrlKey?: boolean; shiftKey?: boolean }): KeyEventStub {
  return new KeyEventStub({ type: 'keydown', ...opts });
}

describe('L3 生产按键分发 — keyEventHandler（TQ-E-02）', () => {
  beforeAll(() => {
    // L3 node 环境无 window——ShortcutRegistry 注册/重置命令会装拆 window keydown 监听
    //（ensureListenerInstalled/Removed）。stub 全程持有（不得按用例拆装：_reset 的
    // ensureListenerRemoved 在下一次 _reset 时触发，用后即拆会踩 window undefined）
    vi.stubGlobal('window', { addEventListener: vi.fn(), removeEventListener: vi.fn() });
  });

  afterAll(() => {
    vi.unstubAllGlobals();
  });

  beforeEach(() => {
    getShortcutRegistry()._reset();
  });

  afterEach(() => {
    getShortcutRegistry()._reset();
  });

  it('Ctrl+Shift+C（terminal.copy 已注册）→ 返回 false 且 preventDefault（被注册表消费）', () => {
    getShortcutRegistry().register([commandFromMeta('terminal.copy', () => true)]);
    const event = keyDown({ code: 'KeyC', ctrlKey: true, shiftKey: true });
    const result = handleTerminalKeyEvent(event as unknown as KeyboardEvent);
    expect(result).toBe(false);
    expect(event.defaultPrevented).toBe(true);
  });

  it('未注册键（KeyA 无修饰）→ 返回 true 且不 preventDefault（透传）', () => {
    getShortcutRegistry().register([commandFromMeta('terminal.copy', () => true)]);
    const event = keyDown({ code: 'KeyA' });
    const result = handleTerminalKeyEvent(event as unknown as KeyboardEvent);
    expect(result).toBe(true);
    expect(event.defaultPrevented).toBe(false);
  });

  it('非 keydown 类型（keyup）→ 直接透传 true（不查注册表）', () => {
    getShortcutRegistry().register([commandFromMeta('terminal.copy', () => true)]);
    const event = new KeyEventStub({ type: 'keyup', code: 'KeyC', ctrlKey: true, shiftKey: true });
    const result = handleTerminalKeyEvent(event as unknown as KeyboardEvent);
    expect(result).toBe(true);
    expect(event.defaultPrevented).toBe(false);
  });

  it('Shift+Tab：terminal context 无绑定且为保留键（不可绑）→ 透传 true（xterm 反缩进）', () => {
    getShortcutRegistry().register([commandFromMeta('terminal.copy', () => true)]);
    const event = keyDown({ code: 'Tab', shiftKey: true });
    const result = handleTerminalKeyEvent(event as unknown as KeyboardEvent);
    expect(result).toBe(true);
    expect(event.defaultPrevented).toBe(false);
  });
});
