// large-file-viewer.test.tsx — LargeFileViewer / useLineIndex / blockCache L2 测试（CP-022）
//
// 覆盖（checklist CP-022 步骤 4 + FE-03/04/05）:
// - 组件级: mock ipc/fs.readFileRange 虚拟大文件——首块索引渲染 / 窗口渲染行数与
//   滚动位置 / LRU 驱逐（回滚旧块触发重读）/ 信息条口径 / 失败兜底
// - FE-03: 方案切换后行文本色响应更新（schemeRegistry.onDidChange 订阅）
// - FE-04: 信息条大小 = fs_stat 真实字节（stat 失败降级 …）
// - FE-05: 外部修改失效——invalidateFile 清缓存/代际防脏回填/fs-event 命中重扫/未变不失效
// - useLineIndex 单测: 首块索引行数 / 跨块行拼接 / 多字节字符边界
//
// mock readFileRange 语义 = 后端 fs_read_file_range 契约简化版（逐字节区间切片）:
// ASCII 内容下区间 [off, off+len) 与后端对齐语义一致,可直接切片。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, waitFor, cleanup, act, fireEvent } from "@testing-library/react";

const { mockReadFileRange, mockStatFile, mockOnFsEvent } = vi.hoisted(() => ({
  mockReadFileRange: vi.fn(),
  mockStatFile: vi.fn(),
  mockOnFsEvent: vi.fn(),
}));

vi.mock("../ipc", () => ({
  fs: { readFileRange: mockReadFileRange, statFile: mockStatFile },
}));

// FE-05: 外部修改订阅——onFsEvent 经 ipc/notify 直连（照 useCodeMirror/DiffPanel 先例）
vi.mock("../ipc/notify", () => ({
  onFsEvent: mockOnFsEvent,
}));

vi.mock("../panels/editor/useCodeMirror", () => ({
  EDITOR_FONT_SPEC: { ".cm-scroller": { fontFamily: "monospace" } },
  MAX_FILE_SIZE_BYTES: 10_000_000,
  LARGE_FILE_WARN_BYTES: 1_000_000,
}));

import { LargeFileViewer, LARGE_FILE_LINE_HEIGHT } from "../panels/editor/largeFileViewer/LargeFileViewer";
import { useLineIndex } from "../panels/editor/largeFileViewer/useLineIndex";
import { renderHook } from "@testing-library/react";
import {
  READ_BLOCK_BYTES,
  BLOCK_CACHE_LIMIT,
  _resetBlockCache,
  readBlock,
  peekCachedBlock,
  invalidateFile,
} from "../panels/editor/largeFileViewer/blockCache";
import { schemeRegistry } from "../theme/schemeRegistry";
import { linear } from "../theme/schemes/linear";
import type { ColorScheme } from "../theme/schemes/types";

// ── 虚拟文件夹具 ──────────────────────────────────────────────
//
// 以行内容为 chunk 构造整文件文本（不越过块边界 256KB 的 ASCII 文本,
// mock 切片即后端契约响应）。每行 "line-0000000" 形态 12 字节（含 \n）?

/** 生成 n 行、每行 "line-<idx padded>" 的文本（含行尾 \n） */
function makeLinesText(fromLine: number, count: number): string {
  const parts: string[] = [];
  for (let i = fromLine; i < fromLine + count; i++) {
    parts.push(`line-${String(i).padStart(7, "0")}\n`);
  }
  return parts.join("");
}

/** 行字节长度（含 \n）——makeLinesText 单行定长: "line-0000000\n" = 13 字节 */
const LINE_BYTES = 13;

/** 装配按字节切片的 readFileRange mock（ASCII 简化契约）;返回总字节数 */
function installVirtualFile(lines: number): number {
  // 惰性构造: 仅生成被请求区间所在行段,避免一次性构造超大字符串
  const totalBytes = lines * LINE_BYTES;
  mockReadFileRange.mockImplementation((_path: string, offset: number, length: number) => {
    if (offset >= totalBytes) return Promise.resolve("");
    const end = Math.min(offset + length, totalBytes);
    // 定位覆盖行段并切片
    const lineFrom = Math.floor(offset / LINE_BYTES);
    const lineTo = Math.ceil(end / LINE_BYTES);
    const raw = makeLinesText(lineFrom, lineTo - lineFrom);
    const localStart = offset - lineFrom * LINE_BYTES;
    const localEnd = end - lineFrom * LINE_BYTES;
    return Promise.resolve(raw.slice(localStart, localEnd));
  });
  return totalBytes;
}

/** 查询某行文本（data-line 锚） */
function lineText(container: HTMLElement, lineIndex: number): string | null {
  const el = container.querySelector(`[data-line="${lineIndex}"]`);
  return el ? el.textContent : null;
}

/** 行文本色期望值——jsdom 对 style.color 做序列化（探针元素同源归一化） */
function expectedColor(hex: string): string {
  const probe = document.createElement("div");
  probe.style.color = hex;
  return probe.style.color;
}

beforeEach(() => {
  mockReadFileRange.mockReset();
  mockStatFile.mockReset();
  mockStatFile.mockResolvedValue({ sizeBytes: 0, mtimeMs: null });
  mockOnFsEvent.mockReset();
  mockOnFsEvent.mockReturnValue(() => {});
  _resetBlockCache();
  // FE-03: 行文本色取 schemeRegistry.getActive()——_reset 清表后须重注册 linear
  schemeRegistry._reset();
  schemeRegistry.register(linear);
});

afterEach(() => {
  cleanup();
});

describe("LargeFileViewer", () => {
  const FILE = "D:/big/log.txt";

  it("渲染信息条（只读浏览 + fs_stat 真实大小 + 可编辑上限 10MB + 来源标签）", async () => {
    installVirtualFile(200);
    // FE-04: 信息条大小 = fs_stat 真实字节（原 size 近似 prop 已删）
    mockStatFile.mockResolvedValue({ sizeBytes: 2_500_000, mtimeMs: 1 });
    const { container } = render(<LargeFileViewer filePath={FILE} sourceLabel="git show" />);
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="lfv-info-bar"]')).toBeTruthy();
    });
    expect(container.querySelector('[data-e2e="large-file-viewer"]')).toBeTruthy();
    const bar = container.querySelector('[data-e2e="lfv-info-bar"]')!;
    expect(bar.textContent).toContain("git show");
    expect(bar.textContent).toContain("只读浏览");
    expect(bar.textContent).toContain("可编辑上限 10MB");
    await waitFor(() => {
      expect(bar.textContent).toContain("2.5MB");
    });
  });

  it("statFile 失败时信息条降级为 …（FE-04）", async () => {
    installVirtualFile(10);
    mockStatFile.mockRejectedValue(new Error("ENOENT"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { container } = render(<LargeFileViewer filePath={FILE} sourceLabel="" />);
    const bar = container.querySelector('[data-e2e="lfv-info-bar"]')!;
    expect(bar.textContent).toContain("只读浏览（…）");
    await waitFor(() => {
      expect(warnSpy).toHaveBeenCalled();
    });
    // stat 失败后维持降级展示（不显示 0.0MB 假值）
    expect(bar.textContent).toContain("只读浏览（…）");
    warnSpy.mockRestore();
  });

  it("方案切换后行文本色响应更新（FE-03）", async () => {
    installVirtualFile(50);
    const alt: ColorScheme = {
      ...linear,
      id: "alt",
      label: "Alt",
      editor: {
        ...linear.editor,
        overrides: { ...linear.editor.overrides, plainText: "#123456" },
      },
    };
    schemeRegistry.register(alt);
    const { container } = render(<LargeFileViewer filePath={FILE} sourceLabel="" />);
    await waitFor(() => {
      expect(lineText(container, 0)).toBe("line-0000000");
    });
    const rowColor = () =>
      (container.querySelector('[data-line="0"]') as HTMLElement).style.color;
    expect(rowColor()).toBe(expectedColor(linear.editor.overrides.plainText));
    await act(async () => {
      schemeRegistry.setActive("alt");
    });
    expect(rowColor()).toBe(expectedColor("#123456"));
  });

  it("首块索引扩展: 行数增长至覆盖探针目标,首行文本正确（mock 按块请求）", async () => {
    installVirtualFile(5000);
    const { container } = render(<LargeFileViewer filePath={FILE} sourceLabel="" />);
    // 探针推进: 行索引持续扩展——至少覆盖探针行（视口 0 高度时 probeRow 恒 21）
    await waitFor(() => {
      expect(lineText(container, 0)).toBe("line-0000000");
    });
    // 请求以 256KB 块边界发起
    expect(mockReadFileRange).toHaveBeenCalledWith(FILE, 0, READ_BLOCK_BYTES);
  });

  it("窗口渲染行数与滚动位置: 设定视口高度后滚动,窗口行随 scrollTop 平移且行文本正确", async () => {
    installVirtualFile(400);
    const { container } = render(<LargeFileViewer filePath={FILE} sourceLabel="" />);
    const scroller = container.querySelector('[data-e2e="lfv-scroll"]') as HTMLDivElement;
    expect(scroller).toBeTruthy();
    // jsdom 无布局——桩 clientHeight（可见 10 行）
    Object.defineProperty(scroller, "clientHeight", { value: 10 * LARGE_FILE_LINE_HEIGHT, configurable: true });

    // 滚动到第 50 行: scrollTop = 50*20
    await act(async () => {
      fireEvent.scroll(scroller, { target: { scrollTop: 50 * LARGE_FILE_LINE_HEIGHT } });
      await new Promise((r) => setTimeout(r, 10));
    });

    // 窗口行: 可见 10 行 + overscan 20 上下——断言窗口内首行数据-line=30（50-20）
    await waitFor(() => {
      const rows = Array.from(scroller.querySelectorAll("[data-line]")).map((el) =>
        Number((el as HTMLElement).getAttribute("data-line")),
      );
      expect(rows.length).toBeGreaterThan(0);
      expect(rows[0]).toBe(30);
      expect(lineText(container, 30)).toBe("line-0000030");
    });
    // 窗口外行（视口外远行）不渲染（虚拟化生效）
    const el50 = scroller.querySelector('[data-line="50"]');
    // 行 50 在窗口内（可见 50..60+20）——用行 0 断言窗口外不渲染（0 < 30-20=10? 0 在 overscan 外）
    const el0 = scroller.querySelector('[data-line="0"]');
    expect(el0).toBeNull();
    void el50;
  });
});

describe("文件变更失效（FE-05）", () => {
  const FILE = "D:/big/watch.txt";

  it("invalidateFile 清空该文件全部缓存块且不影响他文件", async () => {
    mockReadFileRange.mockImplementation((path: string) =>
      Promise.resolve(path === FILE ? "aaa" : "bbb"),
    );
    await readBlock(FILE, 0);
    await readBlock(FILE, 1);
    await readBlock("D:/big/other.txt", 0);
    expect(peekCachedBlock(FILE, 0)).toBe("aaa");
    invalidateFile(FILE);
    expect(peekCachedBlock(FILE, 0)).toBeUndefined();
    expect(peekCachedBlock(FILE, 1)).toBeUndefined();
    // 前缀隔离: 他文件同前缀路径不受影响
    expect(peekCachedBlock("D:/big/other.txt", 0)).toBe("bbb");
  });

  it("读取途中失效的旧代际结果不回填缓存（调用方仍收响应）", async () => {
    let resolveRead!: (v: string) => void;
    mockReadFileRange.mockImplementation(
      () => new Promise<string>((resolve) => (resolveRead = resolve)),
    );
    const pending = readBlock(FILE, 0);
    expect(mockReadFileRange).toHaveBeenCalledTimes(1);
    // 读取在途时文件失效（代际推进）
    invalidateFile(FILE);
    resolveRead("stale");
    await expect(pending).resolves.toBe("stale");
    expect(peekCachedBlock(FILE, 0)).toBeUndefined();
  });

  it("fs-event Modify 命中后索引复位重建（重 stat 变化 → 缓存失效 + 重扫）", async () => {
    installVirtualFile(200);
    mockStatFile.mockResolvedValue({ sizeBytes: 200 * LINE_BYTES, mtimeMs: 1000 });
    let fireFsEvent: ((e: { paths: string[]; kind: string; detail: string }) => void) | null =
      null;
    mockOnFsEvent.mockImplementation((cb: (e: { paths: string[]; kind: string; detail: string }) => void) => {
      fireFsEvent = cb;
      return () => {};
    });
    const { container } = render(<LargeFileViewer filePath={FILE} sourceLabel="" />);
    await waitFor(() => {
      expect(lineText(container, 0)).toBe("line-0000000");
    });
    await waitFor(() => {
      expect(fireFsEvent).not.toBeNull();
    });
    // 外部修改: mtime 变化（事件路径为 Windows 反斜杠形态——归一化比较须命中）
    mockStatFile.mockResolvedValue({ sizeBytes: 200 * LINE_BYTES, mtimeMs: 2000 });
    mockReadFileRange.mockClear();
    await act(async () => {
      fireFsEvent!({ paths: [FILE.replace(/\//g, "\\")], kind: "Modify", detail: "Data" });
    });
    // 失效 → 行索引复位 → 重新拉取块 0 重扫
    await waitFor(() => {
      expect(mockReadFileRange).toHaveBeenCalledWith(FILE, 0, READ_BLOCK_BYTES);
    });
    await waitFor(() => {
      expect(lineText(container, 0)).toBe("line-0000000");
    });
  });

  it("mtime/size 未变的事件不失效（缓存保留,零重读）", async () => {
    installVirtualFile(200);
    mockStatFile.mockResolvedValue({ sizeBytes: 200 * LINE_BYTES, mtimeMs: 1000 });
    let fireFsEvent: ((e: { paths: string[]; kind: string; detail: string }) => void) | null =
      null;
    mockOnFsEvent.mockImplementation((cb: (e: { paths: string[]; kind: string; detail: string }) => void) => {
      fireFsEvent = cb;
      return () => {};
    });
    const { container } = render(<LargeFileViewer filePath={FILE} sourceLabel="" />);
    await waitFor(() => {
      expect(lineText(container, 0)).toBe("line-0000000");
    });
    await waitFor(() => {
      expect(fireFsEvent).not.toBeNull();
    });
    mockReadFileRange.mockClear();
    await act(async () => {
      fireFsEvent!({ paths: [FILE], kind: "Modify", detail: "Data" });
      await new Promise((r) => setTimeout(r, 20));
    });
    // 同 mtime/size → 不失效: 无重读、行文本保留
    expect(mockReadFileRange).not.toHaveBeenCalled();
    expect(lineText(container, 0)).toBe("line-0000000");
  });

  it("非 Modify 类事件不触发失效比对（Create/Remove 不重 stat）", async () => {
    installVirtualFile(50);
    let fireFsEvent: ((e: { paths: string[]; kind: string; detail: string }) => void) | null =
      null;
    mockOnFsEvent.mockImplementation((cb: (e: { paths: string[]; kind: string; detail: string }) => void) => {
      fireFsEvent = cb;
      return () => {};
    });
    const { container } = render(<LargeFileViewer filePath={FILE} sourceLabel="" />);
    await waitFor(() => {
      expect(lineText(container, 0)).toBe("line-0000000");
    });
    await waitFor(() => {
      expect(fireFsEvent).not.toBeNull();
    });
    const statCalls = mockStatFile.mock.calls.length;
    await act(async () => {
      fireFsEvent!({ paths: [FILE], kind: "Create", detail: "Any" });
      await new Promise((r) => setTimeout(r, 20));
    });
    expect(mockStatFile.mock.calls.length).toBe(statCalls);
  });
});

describe("useLineIndex 单测", () => {
  const FILE = "D:/big/idx.txt";
  /** 装配 ASCII 虚拟文件: 每行定长;返回总字节数 */
  function install(fileBytesText: string): number {
    const totalBytes = fileBytesText.length;
    mockReadFileRange.mockImplementation((_path: string, offset: number, length: number) => {
      if (offset >= totalBytes) return Promise.resolve("");
      return Promise.resolve(fileBytesText.slice(offset, offset + length));
    });
    return totalBytes;
  }

  it("首块索引: lineCount 覆盖首块内行数,行文本与源一致", async () => {
    const text = "alpha\nbeta\ngamma\n";
    install(text);
    const { result } = renderHook(() => useLineIndex(FILE, 0));
    // 索引为按需扩展（消费方 getLine 探测驱动）——越界探测推进至 EOF
    await waitFor(() => {
      result.current.getLine(3); // 越界探测 → 触发扫描扩展
      expect(result.current.lineCount).toBeGreaterThanOrEqual(3);
      expect(result.current.getLine(0)).toBe("alpha");
      expect(result.current.getLine(1)).toBe("beta");
      expect(result.current.getLine(2)).toBe("gamma");
      // EOF 已确认（越界探测读到空响应）——行数精确
      expect(result.current.fullyIndexed).toBe(true);
    });
  });

  it("跨块行拼接: 单行超出块边界时行文本跨两块完整重组", async () => {
    // 行 = 300KB 'x' + \n——跨 READ_BLOCK_BYTES 边界;块 0 无 \n,块 1 内含行终结
    const lineLen = READ_BLOCK_BYTES + 40 * 1024;
    const text = "x".repeat(lineLen) + "\n" + "tail\n";
    install(text);
    const { result } = renderHook(() => useLineIndex(FILE, 0));
    await waitFor(() => {
      result.current.getLine(2); // 探测推进（跨块行所在块 + EOF）
      expect(result.current.getLine(0)).toBe("x".repeat(lineLen));
      expect(result.current.getLine(1)).toBe("tail");
      expect(result.current.lineCount).toBeGreaterThanOrEqual(2);
      expect(result.current.fullyIndexed).toBe(true);
    });
  });

  it("CRLF 行尾: 行文本剥除 \\r", async () => {
    const text = "a\r\nb\r\n";
    install(text);
    const { result } = renderHook(() => useLineIndex(FILE, 0));
    await waitFor(() => {
      result.current.getLine(5); // 越界探测 → 扫描至 EOF
      expect(result.current.fullyIndexed).toBe(true);
    });
    expect(result.current.getLine(0)).toBe("a");
    expect(result.current.getLine(1)).toBe("b");
  });

  it("多字节字符（UTF-8）行内容不截断", async () => {
    // 每行中文 + \n;ASCII 切片契约下区间不会切在字符中段（每行 4 字节对齐? 3+1=4——块边界 262144 可整除 4,不跨界）
    const line = "汉字x\n"; // 3+3+1+1=8 字节
    const text = line.repeat(1000);
    install(text);
    const { result } = renderHook(() => useLineIndex(FILE, 0));
    await waitFor(() => {
      result.current.getLine(1000); // 探测推进
      expect(result.current.lineCount).toBeGreaterThanOrEqual(1000);
      expect(result.current.getLine(0)).toBe("汉字x");
      expect(result.current.getLine(1)).toBe("汉字x");
      expect(result.current.getLine(2)).toBe("汉字x");
      expect(result.current.fullyIndexed).toBe(true);
    });
  });

  it("fileRev 递增 → 索引复位重扫（FE-05）", async () => {
    install("alpha\nbeta\n");
    const { result, rerender } = renderHook(
      ({ rev }: { rev: number }) => useLineIndex(FILE, rev),
      { initialProps: { rev: 0 } },
    );
    await waitFor(() => {
      result.current.getLine(5); // 越界探测 → 扫描至 EOF
      expect(result.current.fullyIndexed).toBe(true);
      expect(result.current.lineCount).toBe(2);
    });
    rerender({ rev: 1 });
    // 复位后回到未索引态（lineCount 重新从 1 起）——探测再次推进至 EOF
    expect(result.current.lineCount).toBe(1);
    await waitFor(() => {
      result.current.getLine(5); // 复位后再次越界探测 → 重扫
      expect(result.current.fullyIndexed).toBe(true);
      expect(result.current.getLine(0)).toBe("alpha");
    });
  });

  it("块读取失败 → fatalError 提示且停止扩展（不再重复请求）", async () => {
    mockReadFileRange.mockRejectedValue(new Error("路径不存在"));
    const { result } = renderHook(() => useLineIndex(FILE, 0));
    await waitFor(() => {
      result.current.getLine(0); // 触发首块扫描 → 读失败
      expect(result.current.fatalError).not.toBeNull();
    });
    expect(result.current.fatalError).toContain("路径不存在");
    const callCount = mockReadFileRange.mock.calls.length;
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
    expect(mockReadFileRange.mock.calls.length).toBe(callCount); // 失败块不重试
  });

  it("LRU 驱逐: 缓存上限外旧块被逐出（回滚到首块触发重读）", async () => {
    install(makeLinesText(0, 1)); // 占位——下面重装配
    // 装配多块文件: 每块内容行号互异（行跨块,块内独立可查）
    const blocksNeeded = BLOCK_CACHE_LIMIT + 8;
    const linesPerBlock = Math.floor(READ_BLOCK_BYTES / LINE_BYTES) - 1; // 每块约 20K 行
    const totalLines = blocksNeeded * linesPerBlock;
    // 覆盖 readFileRange 全量: 生成文本可能大——逐块惰性切
    const byteLen = totalLines * LINE_BYTES;
    mockReadFileRange.mockImplementation((_path: string, offset: number, length: number) => {
      if (offset >= byteLen) return Promise.resolve("");
      const lineFrom = Math.floor(offset / LINE_BYTES);
      const lineTo = Math.ceil(Math.min(offset + length, byteLen) / LINE_BYTES);
      const raw = makeLinesText(lineFrom, lineTo - lineFrom);
      return Promise.resolve(raw.slice(offset - lineFrom * LINE_BYTES, Math.min(offset + length, byteLen) - lineFrom * LINE_BYTES));
    });

    const { result } = renderHook(() => useLineIndex(FILE, 0));
    // 拉到中部块（远超缓存上限）→ 首块被 LRU 逐出
    const midLine = blocksNeeded * linesPerBlock - 2;
    await waitFor(() => {
      expect(result.current.getLine(midLine)).toBe(`line-${String(midLine).padStart(7, "0")}`);
    }, { timeout: 3000 });
    // 回滚到行 1 → 首块文本需重新经 IPC 拉取（缓存已逐出）
    mockReadFileRange.mockClear();
    await waitFor(() => {
      expect(result.current.getLine(1)).toBe(`line-${String(1).padStart(7, "0")}`);
    });
    expect(mockReadFileRange).toHaveBeenCalledWith(FILE, 0, READ_BLOCK_BYTES);
  });
});
