import { randomFillSync } from 'node:crypto';
import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

// ─── 全局 mock 工厂（供 vi.hoisted() 中使用） ───
// vi.hoisted() 运行在 ESM import 之前，无法通过 import/require 加载外部模块，
// 因此将共享工厂注册到 globalThis，使其在 vi.hoisted() 回调中可用。

type Fn = ReturnType<typeof vi.fn>;

interface FsMockOverrides {
  readDir?: Fn; readFile?: Fn; createDir?: Fn; deleteEntry?: Fn; rename?: Fn; writeFile?: Fn;
}

function createFsMocks(overrides?: FsMockOverrides) {
  return {
    readDir: overrides?.readDir ?? vi.fn().mockResolvedValue([]),
    readFile: overrides?.readFile ?? vi.fn(),
    createDir: overrides?.createDir ?? vi.fn(),
    deleteEntry: overrides?.deleteEntry ?? vi.fn(),
    rename: overrides?.rename ?? vi.fn(),
    writeFile: overrides?.writeFile ?? vi.fn(),
  };
}

function createGitMocks(overrides?: { gitStatus?: Fn }) {
  return { gitStatus: overrides?.gitStatus ?? vi.fn().mockResolvedValue([]) };
}

function createNotifyMocks(overrides?: { startWatch?: Fn; onFsEvent?: Fn }) {
  let fsEventCallback: (() => void) | null = null;
  return {
    startWatch: overrides?.startWatch ?? vi.fn().mockResolvedValue(undefined),
    onFsEvent: overrides?.onFsEvent ?? vi.fn((cb: () => void) => {
      fsEventCallback = cb;
      return () => { fsEventCallback = null; };
    }),
    triggerFsEvent() { fsEventCallback?.(); },
  };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
(globalThis as any).__createFsMocks = createFsMocks;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(globalThis as any).__createGitMocks = createGitMocks;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(globalThis as any).__createNotifyMocks = createNotifyMocks;

// 类型声明以消除 TS 报错
declare global {
  var __createFsMocks: typeof createFsMocks;
  var __createGitMocks: typeof createGitMocks;
  var __createNotifyMocks: typeof createNotifyMocks;
}

// 静音 jsdom HTMLCanvasElement.getContext 未实现警告
// detectWebgl.test.ts 通过 vi.spyOn 覆盖此 mock（mockRestore 恢复到本 mock）
vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);

// mock ipc/notify（useFileTree / useCodeMirror 在 import 时调用 onFsEvent）
vi.mock("../ipc/notify", () => ({
  onFsEvent: () => () => {},
  startWatch: () => Promise.resolve(),
}));

// jsdom 缺少 crypto.getRandomValues（xterm.js 等库依赖）
Object.defineProperty(window, 'crypto', {
  value: {
    getRandomValues: (buffer: Uint8Array) => randomFillSync(buffer),
  },
});

// jsdom 缺少 ResizeObserver（Dockview 依赖）
if (typeof global.ResizeObserver === 'undefined') {
  global.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

// jsdom 缺少 matchMedia（xterm.js open() 依赖）
if (typeof window.matchMedia !== 'function') {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }),
  });
}

// jsdom 缺少 document.fonts（xterm.js 字体预加载依赖）
// eslint-disable-next-line @typescript-eslint/no-explicit-any
if (typeof (document as any).fonts === 'undefined') {
  Object.defineProperty(document, 'fonts', {
    writable: true,
    value: {
      ready: Promise.resolve(),
    },
  });
}
