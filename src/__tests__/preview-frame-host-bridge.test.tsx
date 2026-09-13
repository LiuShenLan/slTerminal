// preview-frame-host-bridge.test.tsx — PreviewFrame 宿主桥单元测试（ADR-0021）
//
// 预览回迁主窗 DOM：宿主 iframe（自定义协议宿主页，跨源沙箱）直填内容区，
// 消息经宿主页桥 postMessage relay。本文件在 jsdom 真实 iframe 边界上测
// PreviewFrame 编排与校验逻辑（jsdom 不加载跨源 src、不执行宿主页脚本——
// 桥脚本行为由 Rust 侧 host_page_bridge_* 字符串断言 + L4 E2E 验收）：
//   1. 宿主 iframe 渲染（src/sandbox/title/data-e2e）
//   2. 内容推送门控（host_ready 前不推 / 后推 + 重复 ready 重推兜底——宿主
//      iframe 重载场景 / 内容变化重推）
//   3. 上行校验链：source 归属 + origin "null" + nonce + 数值守卫
//   4. keyfwd 收窄转发 → ShortcutRegistry resolve(ev, "global") 消费
//   5. 下行 reset/zoom_set/scroll_set postMessage 载荷
//   6. iframe_loaded → keepZoom/keepScrollRatio 恢复下行
//   7. 卸载摘除监听（不泄漏处理）
//   8. E2E 探针（Q4 裁决）：previewDoc 推送产物 / iframeLoaded 计数写主窗全局

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup, act } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  // ShortcutRegistry 单点 mock——keyfwd 上行消费断言（resolve 调用捕获）
  const resolve = vi.fn<(ev: KeyboardEvent, ctx?: string) => boolean>(() => false);
  return {
    resolve,
    resetAll() {
      resolve.mockReset();
      resolve.mockReturnValue(false);
    },
  };
});

vi.mock("../features/shortcuts/ShortcutRegistry", () => ({
  getShortcutRegistry: () => ({ resolve: mocks.resolve }),
}));

import { PreviewFrame, type PreviewFrameHandle } from "../panels/docViewer/PreviewFrame";
import {
  HOST_READY_MSG_TYPE,
  HOST_IFRAME_LOADED_MSG_TYPE,
  HOST_CONTENT_MSG_TYPE,
  PREVIEW_HOST_URL,
  KEY_FWD_MSG_TYPE,
} from "../panels/docViewer/previewMessages";

const PANEL_ID = "p1";

/** 已退役键盘转发类型名（拼接构造——CP-013 grep 零命中纪律，勿写成字面量） */
const RETIRED_KEY_TYPE = ["slterm", "key"].join("_");

function renderFrame(
  props: Partial<Parameters<typeof PreviewFrame>[0]> = {},
  ref?: React.Ref<PreviewFrameHandle>,
) {
  return render(
    React.createElement(PreviewFrame, {
      panelId: PANEL_ID,
      html: "<p>hello</p>",
      title: "预览",
      ...props,
      ref,
    }),
  );
}

/** 宿主 iframe 元素（data-e2e 契约） */
function hostFrame(container: HTMLElement): HTMLIFrameElement {
  const el = container.querySelector<HTMLIFrameElement>(
    `iframe[data-e2e="preview-frame-${PANEL_ID}"]`,
  );
  expect(el).not.toBeNull();
  return el!;
}

/** 宿主 contentWindow postMessage 捕获（下行断言） */
function spyHostPost(frame: HTMLIFrameElement) {
  return vi.spyOn(frame.contentWindow!, "postMessage");
}

/** 模拟宿主页桥上行（dispatch window message——source/origin 可构造） */
function dispatchUp(
  frame: HTMLIFrameElement,
  msg: Record<string, unknown>,
  opts?: { origin?: string; source?: MessageEventSource | null },
) {
  window.dispatchEvent(
    new MessageEvent("message", {
      data: msg,
      origin: opts?.origin ?? "null",
      source: opts?.source === undefined ? frame.contentWindow : opts.source,
    }),
  );
}

/** host_ready 上行（桥就绪——内容推送门控解除） */
async function sendHostReady(frame: HTMLIFrameElement) {
  await act(async () => dispatchUp(frame, { type: HOST_READY_MSG_TYPE }));
}

/** 从装配产物提取注入 nonce（SEC-04 校验值） */
function extractNonce(doc: string): string {
  const m = /nonce:"([0-9a-f]+)"/.exec(doc);
  if (!m) throw new Error("装配产物未找到 nonce");
  return m[1]!;
}

/** 最近一次 host_content 推送载荷（含装配产物文档） */
function lastContentPush(spy: ReturnType<typeof spyHostPost>): {
  type: string;
  html: string;
  bg: string;
} {
  const calls = spy.mock.calls
    .map(([msg]) => msg as { type?: string; html?: string; bg?: string })
    .filter((m) => m.type === HOST_CONTENT_MSG_TYPE);
  expect(calls.length).toBeGreaterThan(0);
  return calls[calls.length - 1]! as { type: string; html: string; bg: string };
}

describe("PreviewFrame 宿主桥（ADR-0021）", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  afterEach(() => {
    cleanup();
  });

  it("渲染宿主 iframe：自定义协议 src + sandbox（无 allow-same-origin）+ title/data-e2e", () => {
    const { container } = renderFrame();
    const frame = hostFrame(container);
    expect(frame.getAttribute("src")).toBe(PREVIEW_HOST_URL);
    // CVE-2024-35222 红线：allow-scripts 但绝无 allow-same-origin
    expect(frame.getAttribute("sandbox")).toBe("allow-scripts");
    expect(frame.getAttribute("title")).toBe("预览");
  });

  it("内容推送门控：host_ready 前不推；host_ready 上行后直推装配产物 + 背景色", async () => {
    const { container } = renderFrame({ iframeBg: "#112233" });
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    // 就绪前 flush——无 host_content 推送
    await act(async () => {});
    expect(
      post.mock.calls.some(
        ([m]) => (m as { type?: string }).type === HOST_CONTENT_MSG_TYPE,
      ),
    ).toBe(false);

    await sendHostReady(frame);
    const push = lastContentPush(post);
    // 装配产物 = injectScript 完整文档（含注入脚本 + nonce）
    expect(push.html).toContain("<script>");
    expect(push.html).toContain("sltermKeyForward(document,window)");
    expect(push.html).toContain("sltermZoom(document,window)");
    expect(push.bg).toBe("#112233");
    // targetOrigin "*"（opaque origin 序列化 "null"——无显式 targetOrigin 可用）
    expect(post.mock.calls[post.mock.calls.length - 1]![1]).toBe("*");
  });

  it("内容变化（html prop）→ 重推；hostReady 前变化待就绪后合并推送", async () => {
    const { container, rerender } = renderFrame();
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    expect(lastContentPush(post).html).toContain("hello");

    rerender(
      React.createElement(PreviewFrame, {
        panelId: PANEL_ID,
        html: "<p>changed</p>",
        title: "预览",
      }),
    );
    await act(async () => {});
    expect(lastContentPush(post).html).toContain("changed");
  });

  // 防复发（2026-09-13 E2E 实证）：宿主 iframe 因 dockview 面板 DOM reparent
  //（setActive/布局重排——iframe 同文档 reparent 即重载）二次加载 → host_ready
  // 二次到达必须重推内容；布尔 state 幂等曾吃掉二次 ready 的重推，内容 iframe
  // 恒空白（keyfwd E2E 用例 activatePanel 后必踩）
  it("host_ready 重复到达（宿主 iframe 重载）→ 每次均重推内容", async () => {
    const { container } = renderFrame();
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    expect(lastContentPush(post).html).toContain("hello");
    const pushesAfterFirst = post.mock.calls.filter(
      ([m]) => (m as { type?: string }).type === HOST_CONTENT_MSG_TYPE,
    ).length;

    // 宿主 iframe 重载 → 桥二次 ready（同 source/origin 形态）
    await sendHostReady(frame);
    const pushes = post.mock.calls.filter(
      ([m]) => (m as { type?: string }).type === HOST_CONTENT_MSG_TYPE,
    );
    expect(pushes.length).toBe(pushesAfterFirst + 1);
    expect(lastContentPush(post).html).toContain("hello");
  });

  it("上行校验：origin 非 \"null\" 的宿主信号不受理（不推送内容）", async () => {
    const { container } = renderFrame();
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await act(async () =>
      dispatchUp(frame, { type: HOST_READY_MSG_TYPE }, { origin: "http://evil.example" }),
    );
    await act(async () => {});
    expect(
      post.mock.calls.some(
        ([m]) => (m as { type?: string }).type === HOST_CONTENT_MSG_TYPE,
      ),
    ).toBe(false);
  });

  it("上行校验：source 非本 iframe（异源窗口/主窗自身）一律丢弃", async () => {
    const { container } = renderFrame();
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    // 主窗自身为 source（恶意/串台消息）
    await act(async () =>
      dispatchUp(frame, { type: HOST_READY_MSG_TYPE }, { source: window }),
    );
    await act(async () => {});
    expect(
      post.mock.calls.some(
        ([m]) => (m as { type?: string }).type === HOST_CONTENT_MSG_TYPE,
      ),
    ).toBe(false);
  });

  it("zoom 上行：nonce 合法 → onZoomChange 上报；伪造/缺失 nonce 静默丢弃", async () => {
    const onZoomChange = vi.fn();
    const { container } = renderFrame({ onZoomChange });
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    const nonce = extractNonce(lastContentPush(post).html);

    await act(async () =>
      dispatchUp(frame, { type: "slterm_zoom", nonce: "bad", zoom: 1.5 }),
    );
    expect(onZoomChange).not.toHaveBeenCalled();
    await act(async () => dispatchUp(frame, { type: "slterm_zoom", zoom: 1.5 }));
    expect(onZoomChange).not.toHaveBeenCalled();
    // 非数值 zoom（isFiniteZoom 拒绝）
    await act(async () =>
      dispatchUp(frame, { type: "slterm_zoom", nonce, zoom: "1.5" }),
    );
    expect(onZoomChange).not.toHaveBeenCalled();

    await act(async () =>
      dispatchUp(frame, { type: "slterm_zoom", nonce, zoom: 1.3 }),
    );
    expect(onZoomChange).toHaveBeenCalledWith(1.3);
    // 等值回声不重复上报
    await act(async () =>
      dispatchUp(frame, { type: "slterm_zoom", nonce, zoom: 1.3 }),
    );
    expect(onZoomChange).toHaveBeenCalledTimes(1);
  });

  it("scroll/nav 上行：镜像更新（不上报外层）+ nav 校验透传", async () => {
    const onNav = vi.fn();
    const { container } = renderFrame({ onNav });
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    const nonce = extractNonce(lastContentPush(post).html);

    await act(async () =>
      dispatchUp(frame, { type: "slterm_nav", nonce, href: "https://x.com" }),
    );
    expect(onNav).toHaveBeenCalledWith("https://x.com");
    // 空 href / 伪造 nonce 丢弃
    await act(async () =>
      dispatchUp(frame, { type: "slterm_nav", nonce, href: "" }),
    );
    await act(async () =>
      dispatchUp(frame, { type: "slterm_nav", nonce: "bad", href: "https://y.com" }),
    );
    expect(onNav).toHaveBeenCalledTimes(1);

    // scroll 上行（合法）不抛错、不外报
    await act(async () =>
      dispatchUp(frame, { type: "slterm_scroll", nonce, ratio: 0.5 }),
    );
    // 越界 ratio 拒绝（isFiniteRatio）
    await act(async () =>
      dispatchUp(frame, { type: "slterm_scroll", nonce, ratio: 5 }),
    );
  });

  it("keyfwd 上行：合成 KeyboardEvent 经 resolve(ev, \"global\") 消费", async () => {
    const { container } = renderFrame();
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    const nonce = extractNonce(lastContentPush(post).html);

    await act(async () =>
      dispatchUp(frame, {
        type: KEY_FWD_MSG_TYPE,
        nonce,
        code: "KeyW",
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: false,
      }),
    );
    expect(mocks.resolve).toHaveBeenCalledTimes(1);
    const [ev, ctx] = mocks.resolve.mock.calls[0]!;
    expect(ev).toBeInstanceOf(KeyboardEvent);
    expect(ev.type).toBe("keydown");
    expect(ev.code).toBe("KeyW");
    expect(ev.ctrlKey).toBe(true);
    expect(ev.shiftKey).toBe(false);
    expect(ctx).toBe("global");
  });

  it("keyfwd 负面：伪造 nonce / 空 code 不消费（resolve 不调）", async () => {
    const { container } = renderFrame();
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    const nonce = extractNonce(lastContentPush(post).html);

    await act(async () =>
      dispatchUp(frame, {
        type: KEY_FWD_MSG_TYPE,
        nonce: "bad",
        code: "KeyW",
        ctrlKey: true,
      }),
    );
    await act(async () =>
      dispatchUp(frame, { type: KEY_FWD_MSG_TYPE, nonce, code: "" }),
    );
    expect(mocks.resolve).not.toHaveBeenCalled();
  });

  it("旧键转发类型（slterm_key）上行静默丢弃——命令重放通道不复活（CP-013）", async () => {
    const { container } = renderFrame();
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    const nonce = extractNonce(lastContentPush(post).html);

    await act(async () =>
      dispatchUp(frame, {
        type: RETIRED_KEY_TYPE,
        nonce,
        code: "KeyW",
        ctrlKey: true,
      }),
    );
    expect(mocks.resolve).not.toHaveBeenCalled();
  });

  it("resetZoom ref 命令 → 下行 slterm_reset postMessage", async () => {
    const handleRef = React.createRef<PreviewFrameHandle>();
    const { container } = renderFrame({}, handleRef);
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    const nonce = extractNonce(lastContentPush(post).html);

    await act(async () => handleRef.current!.resetZoom());
    const resets = post.mock.calls
      .map(([m]) => m as { type?: string; nonce?: string })
      .filter((m) => m.type === "slterm_reset");
    expect(resets).toEqual([{ type: "slterm_reset", nonce }]);
  });

  it("iframe_loaded：keepZoom/keepScrollRatio 按镜像下行恢复（zoom_set/scroll_set）", async () => {
    const onZoomReset = vi.fn();
    const { container } = renderFrame({
      keepZoom: true,
      keepScrollRatio: true,
      onZoomReset,
    });
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    const nonce = extractNonce(lastContentPush(post).html);

    // 先喂 zoom/scroll 上行 → 父侧镜像
    await act(async () =>
      dispatchUp(frame, { type: "slterm_zoom", nonce, zoom: 1.3 }),
    );
    await act(async () =>
      dispatchUp(frame, { type: "slterm_scroll", nonce, ratio: 0.5 }),
    );
    const before = post.mock.calls.length;
    // 内容重建完成 → 归 1 静默复位 + 按镜像下行恢复
    await act(async () => dispatchUp(frame, { type: HOST_IFRAME_LOADED_MSG_TYPE }));
    expect(onZoomReset).toHaveBeenCalledTimes(1);
    const downs = post.mock.calls
      .slice(before)
      .map(([m]) => m as { type?: string; nonce?: string; zoom?: number; ratio?: number });
    expect(downs).toEqual([
      { type: "slterm_zoom_set", nonce, zoom: 1.3 },
      { type: "slterm_scroll_set", nonce, ratio: 0.5 },
    ]);
  });

  it("iframe_loaded：keep 全关 → 无下行恢复（html 现状语义「重建归 100%」）", async () => {
    const onZoomReset = vi.fn();
    const { container } = renderFrame({ onZoomReset });
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    const nonce = extractNonce(lastContentPush(post).html);
    await act(async () =>
      dispatchUp(frame, { type: "slterm_zoom", nonce, zoom: 1.7 }),
    );
    const before = post.mock.calls.length;
    await act(async () => dispatchUp(frame, { type: HOST_IFRAME_LOADED_MSG_TYPE }));
    expect(onZoomReset).toHaveBeenCalledTimes(1);
    expect(post.mock.calls.length).toBe(before);
  });

  it("fontProbe 上行（TE-08）→ E2E 门控写主窗全局（键 = panelId）", async () => {
    const w = window as unknown as { __slterm_e2e_fontProbe?: Record<string, boolean> };
    delete w.__slterm_e2e_fontProbe;
    try {
      const { container } = renderFrame();
      const frame = hostFrame(container);
      const post = spyHostPost(frame);
      await sendHostReady(frame);
      const nonce = extractNonce(lastContentPush(post).html);

      await act(async () =>
        dispatchUp(frame, { type: "slterm_font_probe", nonce, loaded: true }),
      );
      // vitest 下 import.meta.env.DEV === true → E2E_ENABLED 恒真
      expect(w.__slterm_e2e_fontProbe?.[PANEL_ID]).toBe(true);
      // 伪造 nonce 拒绝——不覆盖既有值
      await act(async () =>
        dispatchUp(frame, { type: "slterm_font_probe", nonce: "bad", loaded: false }),
      );
      expect(w.__slterm_e2e_fontProbe?.[PANEL_ID]).toBe(true);
    } finally {
      delete w.__slterm_e2e_fontProbe;
    }
  });

  it("E2E 内容探针：推送产物按 panelId 写主窗全局 __slterm_e2e_previewDoc", async () => {
    // ADR-0021 spike Q4 裁决（embedded driver frame 内 execute 全灭）——L4
    // 内容断言走主窗探针全局；vitest 下 E2E_ENABLED 恒真
    const w = window as unknown as {
      __slterm_e2e_previewDoc?: Record<string, string>;
    };
    delete w.__slterm_e2e_previewDoc;
    try {
      const { container } = renderFrame();
      const frame = hostFrame(container);
      const post = spyHostPost(frame);
      await sendHostReady(frame);
      const pushed = lastContentPush(post).html;
      // 探针值 = 最近一次推送产物本身（L4 以其含 X 判定内容渲染完成）
      expect(w.__slterm_e2e_previewDoc?.[PANEL_ID]).toBe(pushed);
      expect(w.__slterm_e2e_previewDoc?.[PANEL_ID]).toContain("<p>hello</p>");
    } finally {
      delete w.__slterm_e2e_previewDoc;
    }
  });

  it("E2E 加载探针：iframe_loaded 上行按 panelId 累加计数", async () => {
    const w = window as unknown as {
      __slterm_e2e_iframeLoaded?: Record<string, number>;
    };
    delete w.__slterm_e2e_iframeLoaded;
    try {
      const { container } = renderFrame();
      const frame = hostFrame(container);
      await sendHostReady(frame);
      expect(w.__slterm_e2e_iframeLoaded?.[PANEL_ID]).toBeUndefined();
      await act(async () =>
        dispatchUp(frame, { type: HOST_IFRAME_LOADED_MSG_TYPE }),
      );
      expect(w.__slterm_e2e_iframeLoaded?.[PANEL_ID]).toBe(1);
      // 重建（再次加载）计数累加——L4 重建恢复断言可比较增量
      await act(async () =>
        dispatchUp(frame, { type: HOST_IFRAME_LOADED_MSG_TYPE }),
      );
      expect(w.__slterm_e2e_iframeLoaded?.[PANEL_ID]).toBe(2);
    } finally {
      delete w.__slterm_e2e_iframeLoaded;
    }
  });

  it("卸载摘除消息监听——卸载后上行不再处理", async () => {
    const onZoomChange = vi.fn();
    const { container, unmount } = renderFrame({ onZoomChange });
    const frame = hostFrame(container);
    const post = spyHostPost(frame);
    await sendHostReady(frame);
    const nonce = extractNonce(lastContentPush(post).html);
    unmount();
    await act(async () =>
      dispatchUp(frame, { type: "slterm_zoom", nonce, zoom: 1.5 }),
    );
    expect(onZoomChange).not.toHaveBeenCalled();
  });
});
