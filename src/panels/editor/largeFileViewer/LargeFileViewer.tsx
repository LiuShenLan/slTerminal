// LargeFileViewer.tsx — 大文件只读浏览（CP-022）
//
// 同面板内形态切换组件（不经 panelRegistry 新类型）——editor/gitshow/diff 超限
// 分支改挂本组件: 固定行高虚拟化行窗口（参照 FileTree 手实现虚拟化先例 FE-30）,
// 行内经 useLineIndex 按需读块（blockCache LRU + ipc/fs.readFileRange 分片）。
// 顶部信息条: 来源面板标签 + 「只读浏览(文件大小),可编辑上限 10MB」。
// 容器自持滚动（overflow:auto）;clientHeight 未测得（jsdom/布局异常）时窗口
// 退化为全量渲染兜底（同 FileTree 先例）。

import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useLineIndex } from "./useLineIndex";
import { invalidateFile } from "./blockCache";
import { fs } from "../../../ipc";
import { onFsEvent } from "../../../ipc/notify";
import { EDITOR_BG, ERROR_FG, PANEL_BG, DIM_FG, SEPARATOR_BG } from "../../../theme";
import { schemeRegistry } from "../../../theme/schemeRegistry";
import { EDITOR_FONT_SPEC } from "../useCodeMirror";

/** 固定行高（px）——复用编辑器同款字号规格（默认 14px,行高 1.4 ≈ 20px）;
 *  虚拟化按此行高计算窗口（CP-022 写死,不做字号缩放） */
export const LARGE_FILE_LINE_HEIGHT = 20;
/** 滚动窗口上下 overscan 行数（缓冲渲染,防快速滚动白屏——FileTree 先例为 8,此处放大） */
const OVERSCAN = 20;

/** 行文本字体——复用编辑器同款字体族（EDITOR_FONT_SPEC 单点） */
const LINE_FONT_FAMILY = EDITOR_FONT_SPEC[".cm-scroller"].fontFamily;

/** 人类可读文件大小（MB/GB 一阶精度,信息条展示用） */
function formatSize(bytes: number): string {
  const mb = bytes / 1_000_000;
  if (mb >= 1000) return `${(bytes / 1_000_000_000).toFixed(1)}GB`;
  if (mb >= 10) return `${mb.toFixed(0)}MB`;
  return `${mb.toFixed(1)}MB`;
}

/** 抽样指纹（FE-10）：首/中/末三段各 4KB 文本拼接——同 size 同 mtime 原位改写
 * （FAT 2s 粒度/同毫秒连写）mtime+size 比对假阴性的兜底判据；三段文本直接相等比对。
 * 成本：每次 Modify 事件 ≤12KB 三小段 IPC（fs-event 200ms debounce 天然节流） */
async function sampleFingerprint(filePath: string, sizeBytes: number): Promise<string> {
  const SPAN = 4096;
  const mid = Math.max(0, Math.floor(sizeBytes / 2) - SPAN / 2);
  const tailStart = Math.max(0, sizeBytes - SPAN);
  const [head, middle, tail] = await Promise.all([
    fs.readFileRange(filePath, 0, SPAN),
    fs.readFileRange(filePath, mid, SPAN),
    fs.readFileRange(filePath, tailStart, SPAN),
  ]);
  return `${head}\n${middle}\n${tail}`;
}

/** 大文件只读浏览宿主组件 */
export interface LargeFileViewerProps {
  filePath: string;
  /** 来源面板展示用（editor/gitshow/diff） */
  sourceLabel: string;
}

/**
 * 只读大文件浏览: 固定行高虚拟化行窗口（窗口 = 可见行数 + 上下 overscan 各 20 行),
 * 行内经 useLineIndex 按需读块;顶部信息条提示「只读浏览(文件大小),可编辑上限 10MB」
 */
export const LargeFileViewer: React.FC<LargeFileViewerProps> = ({
  filePath,
  sourceLabel,
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  // 滚动视口状态: scrollTop + 容器高度。
  // height === 0（jsdom 测试环境/布局异常）→ 窗口退化为全量渲染兜底
  const [viewport, setViewport] = useState({ scrollTop: 0, height: 0 });

  // FE-03: 行文本色 = active 方案 editor.overrides.plainText——渲染期取值 +
  // onDidChange 订阅响应式更新（editorThemeSlot 先例照抄；模块级 import 期快照已废）
  const [lineTextColor, setLineTextColor] = useState(
    () => schemeRegistry.getActive().editor.overrides.plainText,
  );
  useEffect(
    () =>
      schemeRegistry.onDidChange(() =>
        setLineTextColor(schemeRegistry.getActive().editor.overrides.plainText),
      ),
    [],
  );

  // FE-04/05: 真实文件元数据（fs_stat）——信息条大小展示 + 失效比对基线
  const [fileMeta, setFileMeta] = useState<{ sizeBytes: number; mtimeMs: number | null } | null>(
    null,
  );
  /** 失效比对基线（stat 基线 ref——首挂 stat 与 fs-event 重 stat 共用一份;FE-10 含抽样指纹） */
  const baseMetaRef = useRef<{ sizeBytes: number; mtimeMs: number | null; fingerprint: string } | null>(
    null,
  );
  /** 文件代际（FE-05）——外部修改命中递增，useLineIndex 按复合键复位重扫 */
  const [fileRev, setFileRev] = useState(0);

  const { lineCount, getLine, fullyIndexed, fatalError, scannedBlocks } = useLineIndex(
    filePath,
    fileRev,
  );

  // FE-09: 索引进度渲染期同步转发到 ref（防首挂 stat then 里闭包过期——
  // fatalError 渲染期直读 ref 同款先例）
  const scannedBlocksRef = useRef(0);
  scannedBlocksRef.current = scannedBlocks;

  // 挂载/换文件 stat 真实元数据（FE-04）：信息条大小展示 + FE-05 失效比对基线
  useEffect(() => {
    let cancelled = false;
    // 换文件先清基线——防旧文件基线误比对新文件事件（stat resolve 前的事件窗口）
    baseMetaRef.current = null;
    fs.statFile(filePath)
      .then(async (m) => {
        if (cancelled) return;
        // FE-10: 基线含抽样指纹（同 size 同 mtime 原位改写假阴性兜底）
        const fp = await sampleFingerprint(filePath, m.sizeBytes);
        if (cancelled) return;
        baseMetaRef.current = { sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs, fingerprint: fp };
        setFileMeta({ sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs });
        // FE-09: 首挂基线竞态封闭——stat resolve 前索引已推进（可能扫到修改前字节）则
        // 保守失效重扫一次（成本一次重扫，换「base null 事件跳过 + 基线后设」窗口封闭；
        // fileRev+1 复位后 scannedBlocks 归零，stat effect deps 仅 [filePath] 不重跑，无循环）
        if (scannedBlocksRef.current > 0) {
          invalidateFile(filePath);
          setFileRev((r) => r + 1);
        }
      })
      .catch((err) =>
        console.warn("[slTerminal] statFile 失败（信息条大小暂缺）:", filePath, err),
      );
    return () => {
      cancelled = true;
    };
  }, [filePath]);

  // FE-05: 外部修改 → 缓存失效 + 行索引复位重扫（只读视图无 dirty，静默重载安全；
  // 事件丢失残余窗口 = 与 editor 域同款 fs-event 依赖，登记 editor/CLAUDE.md）。
  // 事件过滤/路径归一化比较形态照 useCodeMirror.ts:605-635 先例。
  useEffect(() => {
    const off = onFsEvent((event) => {
      if (event.kind !== "Modify") return;
      const normalized = filePath.replace(/\\/g, "/");
      const hit = event.paths.some((p) => p.replace(/\\/g, "/") === normalized);
      if (!hit) return;
      void fs
        .statFile(filePath)
        .then(async (m) => {
          const base = baseMetaRef.current;
          // 基线未就绪（首挂 stat 未归）→ 跳过：同文件不重挂（effect deps [filePath]），
          // 竞态窗口由 FE-09 首挂 resolve 封闭
          if (base === null) return;
          if (m.mtimeMs !== base.mtimeMs || m.sizeBytes !== base.sizeBytes) {
            const fp = await sampleFingerprint(filePath, m.sizeBytes);
            baseMetaRef.current = { sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs, fingerprint: fp };
            setFileMeta({ sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs });
            invalidateFile(filePath);
            setFileRev((r) => r + 1);
            return;
          }
          // FE-10: mtime/size 未变 → 抽样指纹复核（同 size 同 mtime 原位改写假阴性兜底）
          const fp = await sampleFingerprint(filePath, m.sizeBytes);
          if (fp !== base.fingerprint) {
            baseMetaRef.current = { sizeBytes: m.sizeBytes, mtimeMs: m.mtimeMs, fingerprint: fp };
            invalidateFile(filePath);
            setFileRev((r) => r + 1);
          }
        })
        .catch(() => {
          /* stat 失败（文件已删等）——读取路径自行兜底 fatalError */
        });
    });
    return off;
  }, [filePath]);

  // 初始同步测量 + ResizeObserver 跟踪容器高度变化;滚动事件内重测兜底
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const measure = () => {
      setViewport((v) => {
        const h = el.clientHeight;
        return v.height === h ? v : { ...v, height: h };
      });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    setViewport((v) => {
      const h = el.clientHeight;
      return v.scrollTop === el.scrollTop && v.height === h
        ? v
        : { scrollTop: el.scrollTop, height: h };
    });
  }, []);

  // 可见行切片: start/end 各含 OVERSCAN 缓冲行,clamp 到 [0, total]
  const height = viewport.height;
  const total = lineCount;
  const start =
    height > 0
      ? Math.min(Math.max(0, Math.floor(viewport.scrollTop / LARGE_FILE_LINE_HEIGHT) - OVERSCAN), total)
      : 0;
  const end =
    height > 0
      ? Math.min(total, Math.ceil((viewport.scrollTop + height) / LARGE_FILE_LINE_HEIGHT) + OVERSCAN)
      : total;

  // 视口下行需求探针: 视口底部 + overscan 的行索引超出当前索引覆盖 → getLine 探测
  // 触发 useLineIndex 向后扩展（EOF 后 getLine 不再探测,读空循环由 hook 内守卫兜底）;
  // height 未测得（jsdom/布局异常）→ 退化为全量兜底: 持续请求下一行直至 EOF
  const probeRow =
    height > 0
      ? Math.max(0, Math.ceil((viewport.scrollTop + height) / LARGE_FILE_LINE_HEIGHT) + OVERSCAN)
      : total + 1;
  useEffect(() => {
    // 视口底行 + overscan 超出当前索引覆盖 → 请求 useLineIndex 向后扩展
    if (!fullyIndexed) getLine(probeRow);
    // 仅依赖滚动位置与 EOF 态——行数增长经 publish 重渲染后本效应自动重估
  }, [probeRow, fullyIndexed]);

  const rows: number[] = [];
  for (let i = start; i < end; i++) rows.push(i);

  return (
    <div
      data-e2e="large-file-viewer"
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: PANEL_BG,
      }}
    >
      {/* 顶部信息条: 来源面板 + 只读浏览提示（editor/gitshow/diff 超限引导统一口径） */}
      <div
        data-e2e="lfv-info-bar"
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "3px 8px",
          background: PANEL_BG,
          borderBottom: `1px solid ${SEPARATOR_BG}`,
          color: DIM_FG,
          fontSize: 12,
          flexShrink: 0,
          minHeight: 24,
          userSelect: "none",
          whiteSpace: "nowrap",
        }}
      >
        {sourceLabel !== "" && (
          <span style={{ color: lineTextColor, opacity: 0.9 }}>{sourceLabel}</span>
        )}
        <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>
          只读浏览（{fileMeta !== null ? formatSize(fileMeta.sizeBytes) : "…"}），可编辑上限 10MB
        </span>
        {fatalError !== null && (
          <span
            data-e2e="lfv-error"
            style={{ color: ERROR_FG, overflow: "hidden", textOverflow: "ellipsis" }}
            title={fatalError}
          >
            部分内容读取失败
          </span>
        )}
      </div>
      {/* 滚动容器（自持滚动;内容高度 = 行数 × 行高,绝对定位行渲染） */}
      <div
        ref={scrollRef}
        data-e2e="lfv-scroll"
        onScroll={handleScroll}
        style={{
          flex: 1,
          overflow: "auto",
          background: EDITOR_BG,
          fontFamily: LINE_FONT_FAMILY,
          fontSize: 14,
        }}
      >
        <div style={{ height: total * LARGE_FILE_LINE_HEIGHT, position: "relative", width: "max-content", minWidth: "100%" }}>
          {rows.map((i) => (
            <div
              key={i}
              data-line={i}
              style={{
                position: "absolute",
                top: i * LARGE_FILE_LINE_HEIGHT,
                left: 0,
                height: LARGE_FILE_LINE_HEIGHT,
                lineHeight: `${LARGE_FILE_LINE_HEIGHT}px`,
                whiteSpace: "pre",
                color: lineTextColor,
              }}
            >
              {getLine(i) ?? ""}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
