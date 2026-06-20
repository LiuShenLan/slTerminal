// canFit() 五条件守卫——业务行为测试
import { describe, it, expect } from 'vitest';
import { canFit } from '../panels/terminal/useXterm';
import type { Terminal } from '@xterm/xterm';
import type { FitAddon } from '@xterm/addon-fit';

/** 构造一个满足 Terminal 接口的最小 stub */
function terminalStub(opts?: { element?: HTMLElement | null }): Terminal {
  return {
    element: (opts?.element !== undefined ? opts.element : document.createElement('div')),
  } as unknown as Terminal;
}

/** 构造一个满足 FitAddon 接口的最小 stub */
function fitAddonStub(): FitAddon {
  return {} as unknown as FitAddon;
}

/** 构造一个可控制尺寸的 HTMLElement stub */
function containerStub(width: number, height: number): HTMLElement {
  const el = document.createElement('div');
  Object.defineProperty(el, 'offsetWidth', { value: width, writable: true, configurable: true });
  Object.defineProperty(el, 'offsetHeight', { value: height, writable: true, configurable: true });
  return el;
}

describe('canFit 五条件守卫', () => {
  it('terminal 为 null → false', () => {
    expect(canFit(null, fitAddonStub(), containerStub(100, 100), { current: false })).toBe(false);
  });

  it('fitAddon 为 null → false', () => {
    expect(canFit(terminalStub(), null, containerStub(100, 100), { current: false })).toBe(false);
  });

  it('container 为 null → false', () => {
    expect(canFit(terminalStub(), fitAddonStub(), null, { current: false })).toBe(false);
  });

  it('container.offsetWidth 为 0 → false', () => {
    expect(canFit(terminalStub(), fitAddonStub(), containerStub(0, 100), { current: false })).toBe(false);
  });

  it('container.offsetHeight 为 0 → false', () => {
    expect(canFit(terminalStub(), fitAddonStub(), containerStub(100, 0), { current: false })).toBe(false);
  });

  it('terminal.element 为 null（open() 未调用）→ false', () => {
    const term = terminalStub({ element: null });
    expect(canFit(term, fitAddonStub(), containerStub(100, 100), { current: false })).toBe(false);
  });

  it('isDisposedRef.current 为 true → false', () => {
    expect(canFit(terminalStub(), fitAddonStub(), containerStub(100, 100), { current: true })).toBe(false);
  });

  it('全部正常 → true', () => {
    expect(canFit(terminalStub(), fitAddonStub(), containerStub(100, 100), { current: false })).toBe(true);
  });

  it('container.offsetWidth 和 offsetHeight 均为正整数 → true（验证正向路径）', () => {
    expect(canFit(terminalStub(), fitAddonStub(), containerStub(1920, 1080), { current: false })).toBe(true);
  });
});
