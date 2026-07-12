// webgl.ts — WebGL 检测与 WebglAddon 加载（含指数退避重试）
//
// 职责：
// - WebGL2 可用性检测（模块级缓存）
// - WebglAddon 加载 + context loss 后指数退避重试
// - 返回 cancel 函数用于组件卸载时安全清理

import { Terminal } from "@xterm/xterm";
import { WebglAddon } from "@xterm/addon-webgl";

// ---- 常量 ----

/** WebGL context loss 后指数退避重试延迟（ms） */
const WEBGL_RETRY_DELAYS = [1000, 2000, 4000, 8000, 16000];
/** WebGL 最大重试次数 */
const WEBGL_RETRY_MAX = WEBGL_RETRY_DELAYS.length;

// ---- WebGL 检测 ----

/** 模块级 WebGL 检测缓存，避免每次 mount 创建临时 canvas */
let webglCache: boolean | null = null;

/**
 * 检测 WebGL2 是否可用。
 * 首次调用创建临时 canvas 检测并缓存结果，后续调用直接返回缓存值。
 * 生产环境全生命周期内 WebGL 可用性不变，无需重复检测。
 */
export function detectWebgl(): boolean {
  if (webglCache !== null) return webglCache;
  try {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl2", {
      failIfMajorPerformanceCaveat: true,
    });
    webglCache = gl !== null;
  } catch {
    webglCache = false;
  }
  return webglCache;
}

/** 重置 WebGL 检测缓存（仅测试使用，生产环境无需调用） */
export function resetWebglCache(): void {
  webglCache = null;
}

// ---- WebGL 加载与重试 ----

/**
 * 为 Terminal 加载 WebGL addon，context loss 或加载失败时指数退避重试。
 *
 * 返回 `{ cancel }` 对象——调用 cancel() 可立即停止所有重试、清除定时器、
 * 并 dispose 当前 addon。用于组件卸载时安全清理，防止 GPU 资源泄漏。
 *
 * @param term      - xterm.js Terminal 实例
 * @param onSuccess - WebGL addon 成功加载时回调（含 context loss 后重建）
 * @param onFail    - 重试耗尽或 WebGL 不可用时回调（回退 DOM 渲染器）
 * @returns 包含 cancel 函数的对象，调用后停止重试并释放 addon
 */
export function setupWebglWithRetry(
  term: Terminal,
  onSuccess: (addon: WebglAddon) => void,
  onFail: () => void,
): { cancel: () => void } {
  if (!detectWebgl()) {
    onFail();
    return { cancel: () => {} };
  }

  /** 重试定时器集合，cancel 时全部清除 */
  const retryTimers = new Set<ReturnType<typeof setTimeout>>();
  /** 当前加载的 WebglAddon 实例，cancel 时 dispose */
  let currentAddon: WebglAddon | null = null;
  /** 是否已取消（防 cancel 后定时器回调再次执行） */
  let cancelled = false;

  function tryLoad(attempt: number = 0): void {
    if (cancelled) return;

    try {
      const webglAddon = new WebglAddon();
      currentAddon = webglAddon;
      term.loadAddon(webglAddon);
      onSuccess(webglAddon);

      // context loss 时自动 dispose + 指数退避重试
      webglAddon.onContextLoss(() => {
        currentAddon?.dispose();
        currentAddon = null;
        if (cancelled) return;
        if (attempt < WEBGL_RETRY_MAX) {
          const delay = WEBGL_RETRY_DELAYS[attempt];
          console.warn(
            `[WebGL] context loss，${delay}ms 后第 ${attempt + 1}/${WEBGL_RETRY_MAX} 次重建...`,
          );
          const timerId = setTimeout(() => {
            retryTimers.delete(timerId);
            tryLoad(attempt + 1);
          }, delay);
          retryTimers.add(timerId);
        } else {
          console.warn(
            `[WebGL] 已达最大重试次数 (${WEBGL_RETRY_MAX})，回退 DOM 渲染器`,
          );
          onFail();
        }
      });
    } catch {
      // 加载失败（context 可能处于过渡状态），同样指数退避重试
      currentAddon = null;
      if (cancelled) return;
      if (attempt < WEBGL_RETRY_MAX) {
        const delay = WEBGL_RETRY_DELAYS[attempt];
        const timerId = setTimeout(() => {
          retryTimers.delete(timerId);
          tryLoad(attempt + 1);
        }, delay);
        retryTimers.add(timerId);
      } else {
        onFail();
      }
    }
  }

  tryLoad();

  return {
    cancel: () => {
      cancelled = true;
      for (const timerId of retryTimers) {
        clearTimeout(timerId);
      }
      retryTimers.clear();
      currentAddon?.dispose();
      currentAddon = null;
    },
  };
}
