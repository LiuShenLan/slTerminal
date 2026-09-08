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
import { EDITOR_BG, ERROR_FG, PANEL_BG, DIM_FG, SEPARATOR_BG } from "../../../theme";
import { schemeRegistry } from "../../../theme/schemeRegistry";
import { EDITOR_FONT_SPEC } from "../useCodeMirror";

/** 固定行高（px）——复用编辑器同款字号规格（默认 14px,行高 1.4 ≈ 20px）;
 *  虚拟化按此行高计算窗口（CP-022 写死,不做字号缩放） */
export const LARGE_FILE_LINE_HEIGHT = 20;
/** 滚动窗口上下 overscan 行数（缓冲渲染,防快速滚动白屏——FileTree 先例为 8,此处放大） */
const OVERSCAN = 20;

/**
 * 行文本前景色 = active 方案 editor.overrides.plainText（与 CM6 .cm-content 正文同源）
 * ——viewer 非 CM 渲染器,经 schemeRegistry 直取（mdPreviewStyle 先例;colors.ts facade
 * 无编辑器正文 token）。例外登记: panels/editor/CLAUDE.md（CP-022 节）。
 */
const LINE_TEXT_COLOR = schemeRegistry.getActive().editor.overrides.plainText;
/** 行文本字体——复用编辑器同款字体族（EDITOR_FONT_SPEC 单点） */
const LINE_FONT_FAMILY = EDITOR_FONT_SPEC[".cm-scroller"].fontFamily;

/** 人类可读文件大小（MB/GB 一阶精度,信息条展示用） */
function formatSize(bytes: number): string {
  const mb = bytes / 1_000_000;
  if (mb >= 1000) return `${(bytes / 1_000_000_000).toFixed(1)}GB`;
  if (mb >= 10) return `${mb.toFixed(0)}MB`;
  return `${mb.toFixed(1)}MB`;
}

/** 大文件只读浏览宿主组件 */
export interface LargeFileViewerProps {
  filePath: string;
  fileSizeBytes: number;
  /** 来源面板展示用（editor/gitshow/diff） */
  sourceLabel: string;
}

/**
 * 只读大文件浏览: 固定行高虚拟化行窗口（窗口 = 可见行数 + 上下 overscan 各 20 行),
 * 行内经 useLineIndex 按需读块;顶部信息条提示「只读浏览(文件大小),可编辑上限 10MB」
 */
export const LargeFileViewer: React.FC<LargeFileViewerProps> = ({
  filePath,
  fileSizeBytes,
  sourceLabel,
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  // 滚动视口状态: scrollTop + 容器高度。
  // height === 0（jsdom 测试环境/布局异常）→ 窗口退化为全量渲染兜底
  const [viewport, setViewport] = useState({ scrollTop: 0, height: 0 });
  const { lineCount, getLine, fullyIndexed, fatalError } = useLineIndex(
    filePath,
    fileSizeBytes,
  );

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
          <span style={{ color: LINE_TEXT_COLOR, opacity: 0.9 }}>{sourceLabel}</span>
        )}
        <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>
          只读浏览（{formatSize(fileSizeBytes)}），可编辑上限 10MB
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
                color: LINE_TEXT_COLOR,
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

export default LargeFileViewer;
