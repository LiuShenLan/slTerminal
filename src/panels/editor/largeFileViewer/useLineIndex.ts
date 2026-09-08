// useLineIndex.ts — 大文件只读浏览行起始偏移索引 hook（CP-022）
//
// 职责: 把文件字节流切成「行」结构,供 LargeFileViewer 虚拟化行窗口取行文本。
// 与 blockCache 的分工: 块文本 LRU 缓存在 blockCache;本 hook 持有行结构
// （行首字节偏移表）与块文本字节 span 表,二者均从「块序递增读取」派生。
//
// 字节对齐契约（依赖后端 fs_read_file_range 的头回溯/尾裁剪,见 blockCache 头注）:
// 块 b 的响应文本覆盖文件字节 [a_b, e_b), a_0=0 且 a_{b+1}=e_b（零间隙零重叠）——
// a_b = 前序响应文本长度累计。行首偏移 = 块内 \n 本地位置 + a_b + 1。
//
// 索引扩展: 首块扫 \n 建初始索引;滚动至未索引区（行/字节越界）时按需向后扩展
// （单 worker 串行推进,多请求归并目标）。行文本重组: 行可能跨块,按字节区间
// [s, e) 跨块切片拼接;所需块文本不在缓存（LRU 驱逐）→ 异步补载后重渲染。
// 读失败（文件缺失/被删等）: 记录首个错误为 fatalError 停止扩展,防重复 IPC 打满。

import { useEffect, useRef, useState } from "react";
import { peekCachedBlock, readBlock } from "./blockCache";

/** 扫描推进工作区（ref——单线程状态机;渲染快照由此镜像发布） */
interface ScanWorkspace {
  /** 行首字节偏移表（权威真值;snap.starts 为同引用镜像） */
  starts: number[];
  /** 已确认 EOF 的字节位置;null = 未索引到 EOF（行数为下界估计） */
  eofByte: number | null;
  /** 文件是否以 \n 结尾（EOF 确认时末行终点判定用） */
  eofEndsWithNewline: boolean;
  /** 下一个待扫块号 */
  nextBlock: number;
  /** 下一块的文本起点 a（= 前序响应长度累计;与后端对齐契约配套） */
  nextBlockStart: number;
  /** 最近一次响应文本是否以 \n 结尾（EOF 判定行尾形态用） */
  lastTextEndsNL: boolean;
  /** 块文本 span 终点表: spanEnds[b] = 块 b 响应文本终点字节（起点 = b-1 终点 / 0） */
  spanEnds: number[];
  /** 读失败块（停止扩展防重复 IPC;文件级故障多表现为首块即败） */
  failed: Set<number>;
  /** 补载中的块（重渲染去重） */
  loading: Set<number>;
  /** 首个读失败消息（fatalError 展示;null = 无故障） */
  fatal: string | null;
  /** worker 忙碌旗标（防并发推进交错） */
  busy: boolean;
  /** 行覆盖目标（starts.length-1 ≥ 此值即达标） */
  wantLine: number;
  /** 字节覆盖目标（nextBlockStart > 此值即达标;null = 不限制） */
  wantByte: number | null;
  /** 已卸载（worker 停止推进,防卸载后继续 IPC 与 setState） */
  mounted: boolean;
}

/** 新建空白工作区 */
function createWorkspace(): ScanWorkspace {
  return {
    starts: [0],
    eofByte: null,
    eofEndsWithNewline: false,
    nextBlock: 0,
    nextBlockStart: 0,
    lastTextEndsNL: false,
    spanEnds: [],
    failed: new Set(),
    loading: new Set(),
    fatal: null,
    busy: false,
    wantLine: 0,
    wantByte: null,
    mounted: true,
  };
}

/**
 * 行起始偏移索引——首块扫 \n 建初始索引,滚动至未索引区时按需向后扩展
 *
 * 返回成员为契约骨架（CP-022）;fatalError 为附加成员（块读取失败的兜底提示）。
 */
export function useLineIndex(filePath: string, fileSizeBytes: number): {
  /** 总行数（索引未覆盖到 EOF 时为下界估计值） */
  lineCount: number;
  /** 取行文本（虚拟化窗口调用;行所在块未载入则同步触发读块后重渲染） */
  getLine(lineIndex: number): string | undefined;
  /** 索引是否已覆盖到 EOF（行数从估计转精确） */
  fullyIndexed: boolean;
  /** 读取失败兜底提示（文件缺失/删除等;null = 无故障） */
  fatalError: string | null;
} {
  // fileSizeBytes: 调用方（editor/gitshow/diff）以文本 length 近似字节数传入,
  // 仅供宿主信息条展示;索引扩展与 EOF 判定由真实读块响应驱动（CJK 等 length<bytes
  // 场景不受估算误差影响,真实文件长度在读空响应时精确获得）
  void fileSizeBytes;

  const wsRef = useRef<ScanWorkspace>(createWorkspace());
  const [fileKey, setFileKey] = useState(filePath);
  // 渲染快照: 行结构与 EOF 镜像（rev 递增保证每次发布都产生新对象触发重渲染）
  const [snap, setSnap] = useState<{
    starts: number[];
    eofByte: number | null;
    eofEndsWithNewline: boolean;
    rev: number;
  }>(() => ({
    starts: wsRef.current.starts,
    eofByte: null,
    eofEndsWithNewline: false,
    rev: 0,
  }));

  // filePath 变化（查看器实例换文件,如 diff 切换目标文件）→ 全量复位
  if (fileKey !== filePath) {
    setFileKey(filePath);
    wsRef.current = createWorkspace();
    setSnap({
      starts: wsRef.current.starts,
      eofByte: null,
      eofEndsWithNewline: false,
      rev: 0,
    });
  }

  // 挂载/卸载生命周期: 卸载即停 worker（防卸载后继续 IPC 与 setState）
  // 仅生命周期语义——fileKey 变化已在上方渲染期全量复位
  useEffect(() => {
    wsRef.current.mounted = true;
    return () => {
      wsRef.current.mounted = false;
    };
  }, [fileKey]);

  /** 向渲染层发布一次信号（块推进 / EOF / 补载完成均经此 bump——新对象即重渲染） */
  const publish = (): void => {
    const ws = wsRef.current;
    setSnap((s) => ({
      starts: ws.starts,
      eofByte: ws.eofByte,
      eofEndsWithNewline: ws.eofEndsWithNewline,
      rev: s.rev + 1,
    }));
  };

  /** 扫下一块并入索引（推进点唯一）;返回推进结果供 worker 循环判定 */
  const scanNextBlock = async (): Promise<"ok" | "eof" | "stalled"> => {
    const ws = wsRef.current;
    if (ws.eofByte !== null) return "eof"; // EOF 已确认——不再发起读块
    const blockIndex = ws.nextBlock;
    if (ws.failed.has(blockIndex)) return "stalled";
    let text: string;
    try {
      text = await readBlock(filePath, blockIndex);
      // 防御: 契约外响应（undefined 等）按故障停滞处理,防不变量破坏后的失控循环
      if (typeof text !== "string") {
        ws.failed.add(blockIndex);
        if (ws.fatal === null) ws.fatal = "读取文件失败: 异常响应";
        publish();
        return "stalled";
      }
    } catch (err) {
      ws.failed.add(blockIndex);
      if (ws.fatal === null) {
        ws.fatal = err instanceof Error ? err.message : String(err);
        publish(); // 故障上屏（fatalError 经渲染读取,须 bump）
      }
      return "stalled";
    }
    if (!ws.mounted) return "stalled";

    if (text === "") {
      // EOF: 后端约定 offset ≥ 文件长度 → 空串;精确终点 = 上一响应文本终点
      ws.eofByte = ws.nextBlockStart;
      ws.eofEndsWithNewline = ws.lastTextEndsNL;
      // 文件以 \n 结尾 → 其后的幻影行起点无内容,不占行号（末 \n 后无新行）
      const last = ws.starts[ws.starts.length - 1];
      if (ws.eofEndsWithNewline && last === ws.eofByte) {
        ws.starts.pop();
      }
      publish();
      return "eof";
    }

    // 记录块 span（字节对齐契约: 起点 = 前序累计终点;文本 = 响应全量）
    ws.spanEnds.push(ws.nextBlockStart + text.length);
    ws.lastTextEndsNL = text.endsWith("\n");
    // 扫 \n: 本地位置 j → 下一行起点 = a + j + 1（\n 本身属行终结符,不属行文本）
    let from = -1;
    for (;;) {
      const j = text.indexOf("\n", from + 1);
      if (j === -1) break;
      ws.starts.push(ws.nextBlockStart + j + 1);
      from = j;
    }
    ws.nextBlockStart += text.length;
    ws.nextBlock += 1;
    // 统一发布（无 \n 的块行结构未变,但跨块悬垂行的文本终点依赖覆盖区前移）
    publish();
    return "ok";
  };

  /** worker 主循环: 串行消费归并目标（行 / 字节双目标,未达标继续推进） */
  const workerRun = async (): Promise<void> => {
    const ws = wsRef.current;
    try {
      for (;;) {
        if (!ws.mounted || ws.eofByte !== null) break;
        const wantLineOk = ws.starts.length - 1 >= ws.wantLine;
        const wantByteOk = ws.wantByte === null || ws.nextBlockStart > ws.wantByte;
        if (wantLineOk && wantByteOk) break;
        const result = await scanNextBlock();
        if (result !== "ok") break; // EOF / 读失败停滞
      }
    } finally {
      ws.busy = false;
      // 扫描期间可能并入新目标——未达标、未到 EOF 且非故障则再调度一轮（微任务兜底）
      const wantLineOk = ws.starts.length - 1 >= ws.wantLine;
      const wantByteOk = ws.wantByte === null || ws.nextBlockStart > ws.wantByte;
      if (
        ws.mounted &&
        ws.eofByte === null &&
        (!wantLineOk || !wantByteOk) &&
        ws.failed.size === 0
      ) {
        ensureCoverage(ws.wantLine, ws.wantByte);
      }
    }
  };

  /** 请求覆盖扩展（渲染期多探针归并到单 worker;重复请求零成本） */
  const ensureCoverage = (lineTarget: number, byteTarget: number | null = null): void => {
    const ws = wsRef.current;
    if (!ws.mounted) return;
    ws.wantLine = Math.max(ws.wantLine, lineTarget);
    if (byteTarget !== null) {
      ws.wantByte = ws.wantByte === null ? byteTarget : Math.max(ws.wantByte, byteTarget);
    }
    if (ws.busy) return;
    ws.busy = true;
    void workerRun();
  };

  /** 把块 b 文本补载进缓存（行文本重组缺块时触发;完成后 bump 重渲染） */
  const ensureBlockLoaded = (blockIndex: number): void => {
    const ws = wsRef.current;
    if (!ws.mounted || ws.loading.has(blockIndex) || ws.failed.has(blockIndex)) return;
    ws.loading.add(blockIndex);
    void readBlock(filePath, blockIndex)
      .then(() => {
        if (!ws.mounted) return;
        publish(); // 块文本就绪 → 行文本可重组,显式重渲染
      })
      .catch((err) => {
        ws.failed.add(blockIndex);
        if (ws.fatal === null) {
          ws.fatal = err instanceof Error ? err.message : String(err);
          publish();
        }
      })
      .finally(() => {
        ws.loading.delete(blockIndex);
      });
  };

  /** 定位字节所属块号（span 终点表二分）;不在任何 span 内返回 null（未扫描区） */
  const locateBlock = (ws: ScanWorkspace, byte: number): number | null => {
    const ends = ws.spanEnds;
    if (ends.length === 0 || byte < 0 || byte >= ends[ends.length - 1]) return null;
    let lo = 0;
    let hi = ends.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (ends[mid] > byte) hi = mid;
      else lo = mid + 1;
    }
    return lo;
  };

  /** 字节区间 [s, e) 文本重组（跨块切片拼接）;缺块返回 undefined 并触发补载/扫描 */
  const readBytes = (s: number, e: number): string | undefined => {
    if (e <= s) return "";
    const ws = wsRef.current;
    const b0 = locateBlock(ws, s);
    if (b0 === null) {
      ensureCoverage(0, e); // 覆盖区未达该字节 → 请求扫描扩展
      return undefined;
    }
    const b1 = locateBlock(ws, e - 1);
    if (b1 === null) {
      ensureCoverage(0, e);
      return undefined;
    }
    // 所需块文本齐备性检查: 缺任一 → 补载并返回 undefined（就绪后重渲染重组）
    let missing = false;
    for (let b = b0; b <= b1; b++) {
      if (peekCachedBlock(filePath, b) === undefined) {
        ensureBlockLoaded(b);
        missing = true;
      }
    }
    if (missing) return undefined;

    const parts: string[] = [];
    for (let b = b0; b <= b1; b++) {
      const text = peekCachedBlock(filePath, b)!;
      const spanStart = b === 0 ? 0 : ws.spanEnds[b - 1];
      const spanEnd = ws.spanEnds[b];
      const localStart = Math.max(s - spanStart, 0);
      const localEnd = Math.min(e - spanStart, spanEnd - spanStart);
      if (localStart < localEnd) parts.push(text.slice(localStart, localEnd));
    }
    let line = parts.join("");
    // CRLF 行尾: \n 已排除但 \r 留在行文本尾——剥除展示（与 CM6 行归一化语义一致）
    if (line.endsWith("\r")) line = line.slice(0, -1);
    return line;
  };

  /**
   * 取行文本（虚拟化窗口调用;行所在块未载入则同步触发读块后重渲染）
   *
   * 返回 undefined 的场景: 行未索引到（触发向后扩展）/ 末行悬垂未定（等后续 \n 或
   * EOF）/ 行文本所需块补载中——渲染层显示占位,数据就绪后自动重渲染。
   */
  const getLine = (lineIndex: number): string | undefined => {
    if (lineIndex < 0) return undefined;
    const ws = wsRef.current;
    const starts = ws.starts;
    if (lineIndex >= starts.length) {
      // 未索引区 → 请求向后扩展;EOF 已确认时行不存在,不再探测（防空读循环）
      if (ws.eofByte === null) ensureCoverage(lineIndex);
      return undefined;
    }
    const s = starts[lineIndex];
    let e: number;
    if (lineIndex + 1 < starts.length) {
      // 已见终止 \n: 行文本终点 = 下一行起点 - 1
      e = starts[lineIndex + 1] - 1;
    } else {
      // 末行: 文本终点需 EOF 确认（悬垂行到 EOF;文件以 \n 结尾则到该 \n 前）
      if (ws.eofByte === null) {
        ensureCoverage(lineIndex + 1); // 无 EOF → 扩展直至该行终结或 EOF
        return undefined;
      }
      e = ws.eofEndsWithNewline ? ws.eofByte - 1 : ws.eofByte;
    }
    return readBytes(s, e);
  };

  return {
    lineCount: snap.starts.length,
    getLine,
    fullyIndexed: snap.eofByte !== null,
    fatalError: wsRef.current.fatal,
  };
}
