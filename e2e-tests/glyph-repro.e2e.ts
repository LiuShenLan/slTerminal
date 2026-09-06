/**
 * CM 字形绘制丢失取证/防复发 spec（GLYPH_E2E=1 启用，默认跳过零成本）。
 *
 * 背景（2026-09-06 全量取证）：WebView2/Chromium 陈旧光栅缺陷——CM6 文本行
 * 跨帧多次独立输入后部分字形"存在但未绘制"（DOM/几何/字色完整、选中恢复）。
 * 证据链：GPU 合成/光栅/软渲染全关仍复现（G1-G3）、CSS 层提升与 containment
 * 无效（E6 系）、@codemirror/view 6.43.11（tile tree 修复）仍复现、面板结构
 * 无关（txt 也丢）→ 引擎 paint 缓存陈旧。修复 = src/panels/editor/repaintGuard
 * （docChanged 后受影响行 display 往返强制整行重绘，同宏任务）。
 *
 * 本 spec = 修复的像素级防复发锚：真实 GPU 环境跨帧键入（复现矩阵核心场景）
 * 后截图（compositor 取帧）+ PNG 解码判读字形位——guard 被移除/失效即红。
 * 输入通道为 execCommand（embedded WDIO 无 OS 按键通道，承接既有豁免 13 P-15；
 * execCommand 派发 beforeinput → CM6 正常输入事务）。
 *
 * 运行（门控 + 单 spec，e2e-tests/CLAUDE.md 登记）：
 *   node e2e-tests/run-wdio.cjs --spec glyph-repro.e2e.ts
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  createProject,
} from "./specUtils";

/** 顶层门控：GLYPH_E2E 未设 = 整个 describe 跳过（默认套件零成本） */
const glyphActive = !!process.env.GLYPH_E2E;
const describeGlyph = glyphActive ? describe : describe.skip;

/** 截图留档目录（独立于用例临时目录——取证期跨用例积累） */
const SHOT_DIR = join(tmpdir(), "slterm-e2e-glyph-shots");
mkdirSync(SHOT_DIR, { recursive: true });

// ── PNG 解码 + 字形位判读（Node 侧；截图 = compositor 物理像素） ──

/** PNG 解码（支持 colorType 0/2/4/6 + 全滤波）——判读截图的前置 */
function decodePng(path: string): { w: number; h: number; ch: number; px: Uint8Array } {
  const d = readFileSync(path);
  let pos = 8;
  let idat = Buffer.alloc(0);
  let w = 0;
  let h = 0;
  let ch = 0;
  while (pos < d.length) {
    const ln = d.readUInt32BE(pos);
    const typ = d.toString("ascii", pos + 4, pos + 8);
    const data = d.subarray(pos + 8, pos + 8 + ln);
    if (typ === "IHDR") {
      w = data.readUInt32BE(0);
      h = data.readUInt32BE(4);
      ch = { 0: 1, 2: 3, 4: 2, 6: 4 }[data[9]] ?? 0;
    } else if (typ === "IDAT") {
      idat = Buffer.concat([idat, data]);
    } else if (typ === "IEND") {
      break;
    }
    pos += 12 + ln;
  }
  const raw = inflateSync(idat);
  const stride = w * ch;
  const out = new Uint8Array(w * h * ch);
  const prev = new Uint8Array(stride);
  let p = 0;
  for (let y = 0; y < h; y++) {
    const f = raw[p++];
    const line = new Uint8Array(raw.subarray(p, p + stride));
    p += stride;
    if (f === 1) {
      for (let i = ch; i < stride; i++) line[i] = (line[i] + line[i - ch]) & 255;
    } else if (f === 2) {
      for (let i = 0; i < stride; i++) line[i] = (line[i] + prev[i]) & 255;
    } else if (f === 3) {
      for (let i = 0; i < stride; i++) {
        const a = i >= ch ? line[i - ch] : 0;
        line[i] = (line[i] + ((a + prev[i]) >> 1)) & 255;
      }
    } else if (f === 4) {
      for (let i = 0; i < stride; i++) {
        const a = i >= ch ? line[i - ch] : 0;
        const b = prev[i];
        const c = i >= ch ? prev[i - ch] : 0;
        const pa = Math.abs(b - c);
        const pb = Math.abs(a - c);
        const pc = Math.abs(a + b - 2 * c);
        const pr = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
        line[i] = (line[i] + pr) & 255;
      }
    }
    out.set(line, y * stride);
    prev.set(line);
  }
  return { w, h, ch, px: out };
}

/**
 * 判读截图中「行首 run 字形绘制数」。
 * cssRect = 目标行几何（forensics.lineRect，CSS px）；run = 行首 | 个数；
 * dpr = devicePixelRatio（探针实测——截图 = 物理像素）。
 * 方法：y 带内扫描字形段（>70 亮度前景）→ 段起点序列 → advance 中位数 →
 * 文本起点 = 首个字形段起点 → 逐字形位窗口探测命中。
 */
function countGlyphRun(
  shotPath: string,
  cssRect: { x: number; y: number; width: number; height: number },
  run: number,
  dpr: number,
): number {
  const { w, ch, px } = decodePng(shotPath);
  const y0 = Math.floor(cssRect.y * dpr);
  const y1 = Math.ceil((cssRect.y + cssRect.height) * dpr);
  const xStart = Math.floor(cssRect.x * dpr) + 10; // 行容器 padding 前余量
  const scanEnd = Math.min(w, xStart + Math.ceil((run * 24 + 60) * dpr));
  const colHit = (x: number): boolean => {
    for (let y = y0; y < y1; y += 2) {
      const o = (y * w + x) * ch;
      if (ch === 4 && px[o + 3] < 200) continue;
      if (Math.max(px[o], px[o + 1], px[o + 2]) > 70) return true;
    }
    return false;
  };
  // 字形段收集（段宽 ≥2 物理 px）
  const segs: Array<[number, number]> = [];
  let s: number | null = null;
  for (let x = xStart; x < scanEnd; x++) {
    const v = colHit(x);
    if (v && s === null) s = x;
    else if (!v && s !== null) {
      if (x - s >= 2) segs.push([s, x - s]);
      s = null;
    }
  }
  if (s !== null && scanEnd - 1 - s >= 2) segs.push([s, scanEnd - 1 - s]);
  if (segs.length === 0) return 0;
  const starts = segs.map((g) => g[0]);
  const advs: number[] = [];
  for (let i = 0; i < starts.length - 1; i++) advs.push(starts[i + 1] - starts[i]);
  if (advs.length === 0) return 0;
  const adv = [...advs].sort((a, b) => a - b)[Math.floor(advs.length / 2)];
  if (adv < 5) return 0;
  const origin = starts[0];
  let hit = 0;
  for (let i = 0; i < run; i++) {
    const cx = origin + i * adv;
    // 窗口 -2..+14px（相位容差——adv 非整对齐，px 判读实证值）
    for (let k = -2; k < 14; k++) {
      if (colHit(cx + k)) {
        hit++;
        break;
      }
    }
  }
  return hit;
}

// ── 页面内取证通道（helpers.ts 注入 window 全局） ──

interface GlyphWindow {
  __slterm_e2e_cmTypeText?: (
    marker: string,
    text: string,
    mode?: "char" | "batch" | "char-fix",
  ) => { found: boolean; docAfter: string | null };
  __slterm_e2e_cmLineForensics?: (
    marker: string,
    contains: string,
  ) => {
    found: boolean;
    lineText: string | null;
    lineRect: { x: number; y: number; width: number; height: number } | null;
    color: string | null;
    bg: string | null;
    nodes: Array<{
      kind: string;
      text?: string;
      cls?: string;
      rect: { x: number; y: number; width: number; height: number } | null;
    }>;
    visibleCount: number;
  } | null;
}

/** 环境探针：DPI / WebGL renderer（GPU vs SwiftShader）/ UA——"不复现 ≠
 * 修复完成"裁量依据（软渲染/非整数 DPI 环境差异记录在案） */
async function probeEnv(): Promise<{ dpr: number; renderer: string | null; ua: string }> {
  const p = await browser.execute(() => {
    let renderer: string | null = null;
    try {
      const gl = document.createElement("canvas").getContext("webgl");
      if (gl) {
        const ext = gl.getExtension("WEBGL_debug_renderer_info");
        renderer = ext
          ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL))
          : String(gl.getParameter(gl.RENDERER));
      }
    } catch { /* 探针失败不致命 */ }
    return {
      dpr: window.devicePixelRatio,
      renderer,
      ua: navigator.userAgent,
    };
  });
  console.log("[glyph] 环境探针:", JSON.stringify(p));
  return p;
}

/** 打开指定类型面板（md=markdownviewer / html=htmlviewer / txt=editor） */
async function openPanel(
  component: string,
  filePath: string,
  viewMode?: string,
): Promise<void> {
  await browser.execute(
    (args: { component: string; path: string; viewMode?: string }) => {
      const pid = `glyph-${args.component}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      window.__dockviewApi!.addPanel({
        id: pid,
        component: args.component,
        params: {
          panelId: pid,
          filePath: args.path,
          ...(args.viewMode ? { viewMode: args.viewMode } : {}),
        },
      });
    },
    { component, path: filePath, viewMode },
  );
}

/** 等待含 marker 的可见 .cm-content 出现（面板 CM 挂载完成） */
async function waitVisibleCmContaining(marker: string): Promise<void> {
  await browser.waitUntil(
    async () =>
      await browser.execute(
        (m: string) => {
          const vis = Array.from(document.querySelectorAll(".cm-content")).filter(
            (el) => el.getClientRects().length > 0,
          );
          return vis.some((el) => (el.textContent ?? "").includes(m));
        },
        marker,
      ),
    { timeout: 20000, timeoutMsg: `含 marker ${marker} 的可见编辑器未挂载` },
  );
}

/** 键入并断言通道成功（返回插入后全文——通道证据） */
async function typeInto(
  marker: string,
  text: string,
  mode: "char" | "batch" | "char-fix" = "char",
): Promise<string | null> {
  const r = await browser.execute(
    (args: { m: string; t: string; mode: "char" | "batch" | "char-fix" }) => {
      const w = window as unknown as GlyphWindow;
      return w.__slterm_e2e_cmTypeText?.(args.m, args.t, args.mode) ?? null;
    },
    { m: marker, t: text, mode },
  );
  expect(r?.found).toBe(true);
  expect(r?.docAfter).toContain(text);
  return r?.docAfter ?? null;
}

/** DOM 取证快照（行文本/几何/颜色/子节点结构） */
async function forensics(marker: string, contains: string) {
  return browser.execute(
    (args: { m: string; c: string }) => {
      const w = window as unknown as GlyphWindow;
      return w.__slterm_e2e_cmLineForensics?.(args.m, args.c) ?? null;
    },
    { m: marker, c: contains },
  );
}

/** 截图留档并返回路径（compositor 取帧；失败 = 通道不可用证据） */
async function screenshotTo(specLabel: string): Promise<string | null> {
  try {
    const file = join(SHOT_DIR, `${specLabel}-${Date.now()}.png`);
    await browser.saveScreenshot(file);
    console.log(`[glyph] 截图留档: ${file}`);
    return file;
  } catch (err) {
    console.warn(`[glyph] saveScreenshot 不可用（通道证据）: ${String(err)}`);
    return null;
  }
}

/** 面板文件 + marker 创建（marker 唯一文本防跨用例歧义） */
function makeFixture(kind: "md" | "html" | "txt"): {
  dir: string;
  file: string;
  marker: string;
} {
  const dir = mkdtempSync(join(tmpdir(), "slterm-e2e-glyph-"));
  const marker = `glyphbase-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const ext = kind === "md" ? "md" : kind === "html" ? "html" : "txt";
  const file = join(dir, `glyph-${marker}.${ext}`);
  writeFileSync(file, marker + "\n", "utf8");
  return { dir, file, marker };
}

/**
 * 像素防复发断言：截图判读行首 run 字形全绘制（guard 失效即红）。
 * 前置：DOM 层断言已绿（数据完整——bug 只在像素层）。
 */
async function expectGlyphRunDrawn(
  shotPath: string | null,
  f: Awaited<ReturnType<typeof forensics>>,
  run: number,
  dpr: number,
): Promise<void> {
  expect(f?.lineRect?.width ?? 0).toBeGreaterThan(0);
  expect(f?.lineRect?.height ?? 0).toBeGreaterThan(0);
  if (!shotPath) {
    throw new Error("截图通道不可用——像素断言无法执行（saveScreenshot 失败）");
  }
  const hit = countGlyphRun(shotPath, f!.lineRect!, run, dpr);
  if (hit !== run) {
    throw new Error(
      `字形绘制 ${hit}/${run}——repaintGuard 失效或引擎缺陷复发（截图 ${shotPath}）`,
    );
  }
}

/** 开面板前置（目录建项目 + 面板挂载） */
async function setupGlyphArm(kind: "md" | "html" | "txt", dir: string): Promise<void> {
  await waitForWorkspaceReady();
  await createProject(dir);
  await waitForDockviewApi();
}

/** 用例装配：开面板 → 键入 → DOM 断言 → 截图 + 像素断言 */
async function runGlyphArm(
  kind: "md" | "html" | "txt",
  label: string,
  typeBatch: Array<[string, "char" | "batch"]>,
  run: number,
): Promise<void> {
  const env = await probeEnv();
  const { dir, file, marker } = makeFixture(kind);
  try {
    await setupGlyphArm(kind, dir);
    const component =
      kind === "md" ? "markdownviewer" : kind === "html" ? "htmlviewer" : "editor";
    await openPanel(component, file, kind === "html" ? "edit" : undefined);
    await waitVisibleCmContaining(marker);
    for (const [text, mode] of typeBatch) {
      await typeInto(marker, text, mode);
    }
    const f = await forensics(marker, "|".repeat(run));
    console.log(`[glyph][${label}] forensics:`, JSON.stringify(f));
    // DOM 层完整（数据健康硬断言）
    expect(f).not.toBeNull();
    expect(f?.lineText).toContain("|".repeat(run));
    // 像素层（compositor 截图判读——防复发锚）
    await expectGlyphRunDrawn(await screenshotTo(label), f, run, env.dpr);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** 逐字符独立 IPC 键入（每字符一次往返——真实键盘慢打的跨帧近似） */
async function typePerFrame(marker: string, count: number): Promise<void> {
  for (let i = 0; i < count; i++) {
    await typeInto(marker, "|");
  }
}

describeGlyph("CM 字形绘制丢失防复发（GLYPH_E2E=1）", () => {
  it("md：逐字符跨帧键入 5 个 |（用户慢打节奏复现臂）", async () => {
    const env = await probeEnv();
    const { dir, file, marker } = makeFixture("md");
    try {
      await setupGlyphArm("md", dir);
      await openPanel("markdownviewer", file);
      await waitVisibleCmContaining(marker);
      await typePerFrame(marker, 5);
      const f = await forensics(marker, "|||||");
      console.log("[glyph][md5-frame] forensics:", JSON.stringify(f));
      expect(f?.lineText).toContain("|||||");
      await expectGlyphRunDrawn(await screenshotTo("md5-frame"), f, 5, env.dpr);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("md：追加第二组 5 个 |（10 个两批场景——原复现矩阵核心）", async () => {
    await runGlyphArm("md", "md10", [
      ["|||||", "char"],
      ["|||||", "char"],
    ], 10);
  });

  it("html edit：同 10 场景", async () => {
    await runGlyphArm("html", "html10", [
      ["|||||", "char"],
      ["|||||", "char"],
    ], 10);
  });

  it("txt editor 对照：同 10 场景（结构无关性回归）", async () => {
    await runGlyphArm("txt", "txt10", [
      ["|||||", "char"],
      ["|||||", "char"],
    ], 10);
  });

  it("md batch：单事务 5 个 |（对照——单事务本不触发）", async () => {
    await runGlyphArm("md", "md-batch", [["|||||", "batch"]], 5);
  });
});
