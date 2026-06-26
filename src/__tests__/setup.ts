import { randomFillSync } from 'node:crypto';
import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

// mock @tauri-apps/api/event（useFileTree 等模块在 import 时调用 listen）
vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
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
