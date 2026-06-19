import { randomFillSync } from 'node:crypto';

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
