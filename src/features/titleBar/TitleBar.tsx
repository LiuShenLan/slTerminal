// TitleBar —— 自绘窗口标题栏（TB-02 / UI-301）
//
// 定位：tauri.conf.json decorations:false 后由本组件承担原生标题栏职责——
// 拖拽（data-tauri-drag-region="deep" 子树拖拽 + 拖拽区撑满全高，TB-04 / 问题 6 修订）、
// 双击最大化（Tauri 原生拖拽区脚本承担，本组件不注册 onDoubleClick，TB-04 修订）、
// 最小化/最大化/关闭三钮（经 src/ipc/window wrapper，契约见 TB-03）。
// 数据：中段标题 = projects store 活跃项目名 / 活跃页面名
//（layout store 的 activePageId 定位；无现成 selector，直接推导，不改 store）。

import { useShallow } from "zustand/react/shallow";
import { useEffect, useState } from "react";
import type { CSSProperties, FC } from "react";
import type { IconProps } from "../../lib";
import { IconMin, IconMax, IconRestore, IconCloseWin } from "../../lib";
import { minimizeWindow, toggleMaximizeWindow, closeWindow, isWindowMaximized, onWindowResized } from "../../ipc/window";
import { useLayout } from "../../stores/layout";
import { useProjects } from "../../stores/projects";
import {
  TITLEBAR_BG, TITLEBAR_CLOSE_HOVER_BG, SEPARATOR_BG, DIM_FG, SECONDARY_BG, ACCENT_FG, ACTIVE_SELECTION_BG,
} from "../../theme/colors";

/** app logo 终端提示符图形（lucide Terminal path，与设计稿 final-mockup 一致） */
const LOGO_PATH = "M5 8l6 5-6 5M13 19h7";

/** 窗口控制三钮定义（38×26、图标 12px，hover 底 ui.secondaryBg）
 *  TB-07：最大化钮图标/文案随 maximized 态双态切换（IconMax↔IconRestore、
 *  「最大化」↔「还原」），故按 maximized 渲染期派生，不再是模块级静态数组 */
type WinButtonKind = "min" | "max" | "close";
function buildWinButtons(
  maximized: boolean,
): { kind: WinButtonKind; label: string; Icon: FC<IconProps>; onClick: () => void }[] {
  return [
    { kind: "min", label: "最小化", Icon: IconMin, onClick: () => minimizeWindow() },
    {
      kind: "max",
      label: maximized ? "还原" : "最大化",
      Icon: maximized ? IconRestore : IconMax,
      onClick: () => toggleMaximizeWindow(),
    },
    { kind: "close", label: "关闭", Icon: IconCloseWin, onClick: () => closeWindow() },
  ];
}

/**
 * 窗口 maximized 状态感知（TB-07）
 *
 * 初始回查 isWindowMaximized() + 订阅 onWindowResized 事件内回查（onResized 只含
 * PhysicalSize 不含最大化态）。最大化/还原全路径（按钮 / 双击原生拖拽区 / 拖边框 /
 * Win+方向键 snap）必经 resize 事件，覆盖完整——不能只在按钮 onClick 里翻转本地态。
 */
function useWindowMaximized(): boolean {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    let disposed = false;
    const refresh = () => {
      isWindowMaximized()
        .then((v) => {
          if (!disposed) setMaximized(v);
        })
        // 查询失败保持旧值——状态感知降级不阻断标题栏渲染
        .catch(() => {});
    };
    refresh();
    const unlisten = onWindowResized(refresh);
    return () => {
      disposed = true;
      unlisten();
    };
  }, []);
  return maximized;
}

/** 三钮公共样式（hover 底按 kind 覆盖） */
const winButtonBaseStyle: CSSProperties = {
  width: 38,
  height: 26,
  display: "grid",
  placeItems: "center",
  borderRadius: 4,
  border: "none",
  padding: 0,
  background: "transparent",
  color: DIM_FG,
  cursor: "default",
};

/** 从 projects store 推导活跃项目/页面（无现成 selector；禁止改 store） */
function useActiveProjectPage(): { projectName: string | null; pageName: string } {
  const activePageId = useLayout((s) => s.activePageId);
  // 窄订阅（FE-21）：selector 只推导标题所需两原始值字段并经 useShallow 浅比较——
  // 无关项目变更（layout/version/expandedNodes 等）结果浅相等，不触发 TitleBar 重渲染；
  // useShallow 包装器每次渲染重建（无 useCallback 缓存），activePageId 闭包恒最新，
  // 布局切换响应与改造前一致
  return useProjects(
    useShallow((s) => {
      const projList = Object.values(s.projects);
      if (projList.length === 0) return { projectName: null, pageName: "" };
      // 优先按全局活跃页面（layout store）定位所属项目
      if (activePageId) {
        for (const proj of projList) {
          const page = proj.pages.find((p) => p.pageId === activePageId);
          if (page) return { projectName: proj.name, pageName: page.name };
        }
      }
      // layout 无活跃页（未切换过页面/测试环境）：回退第一个项目的 activePageId 页
      const first = projList[0];
      const page = first.pages.find((p) => p.pageId === first.activePageId);
      if (page) return { projectName: first.name, pageName: page.name };
      return { projectName: null, pageName: "" };
    }),
  );
}

export function TitleBar() {
  const { projectName, pageName } = useActiveProjectPage();
  const [hover, setHover] = useState<WinButtonKind | null>(null);
  const maximized = useWindowMaximized();
  const winButtons = buildWinButtons(maximized);

  return (
    <div
      style={{
        height: 34,
        display: "flex",
        alignItems: "center",
        padding: "0 12px", // GL-04：间距收敛 10 → 12
        background: TITLEBAR_BG,
        borderBottom: `1px solid ${SEPARATOR_BG}`,
        fontSize: 12,
        userSelect: "none",
        flexShrink: 0,
      }}
    >
      {/* 左段：app 标识（拖拽区 deep + 撑满全高——裸属性仅命中直接点击元素本身，span/svg
          子元素拦截，deep 使子树内任意处可拖；不撑满时 div 只占内容高度（≈16px 窄带），
          上下死区点不到，TB-04 / 问题 6 修订） */}
      <div
        data-tauri-drag-region="deep"
        style={{
          height: "100%",
          display: "flex", alignItems: "center", gap: 8, color: DIM_FG, fontWeight: 500,
        }}
      >
        <span
          style={{
            width: 16, height: 16, borderRadius: 4,
            background: ACTIVE_SELECTION_BG, // accent-dim 底
            display: "grid", placeItems: "center", color: ACCENT_FG,
          }}
        >
          <svg
            viewBox="0 0 24 24" width={11} height={11}
            fill="none" stroke="currentColor" strokeWidth={2.4} strokeLinecap="round"
            aria-hidden="true"
          >
            <path d={LOGO_PATH} />
          </svg>
        </span>
        slTerminal
      </div>

      {/* 中段：活跃项目名 / 页面名（拖拽区 deep + 撑满全高——无项目时本段为空 div，
          无显式高度时高度 0，点击落点在无 drag 属性的父容器 → 拖不动（问题 6 根因）；
          height 100% 使空 div 占满 34px，flex 居中替代 textAlign 保持文字居中。
          双击最大化由 Tauri 原生拖拽区脚本承担——本组件不注册 onDoubleClick，避免与
          原生 internal_toggle_maximize 双重 toggle（最大化后立即还原），TB-04 修订） */}
      <div
        data-tauri-drag-region="deep"
        style={{
          flex: 1,
          minWidth: 0,
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: DIM_FG,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {projectName && (
          <>
            <span style={{ fontWeight: 500 }}>{projectName}</span>
            <span style={{ margin: "0 8px" }}>/</span>
            <span>{pageName}</span>
          </>
        )}
      </div>

      {/* 右段：窗口控制三钮（不在拖拽区内，保证可点击，TB-04） */}
      <div style={{ display: "flex", gap: 2 }}>
        {winButtons.map(({ kind, label, Icon, onClick }) => {
          const isHovered = hover === kind;
          const isClose = kind === "close";
          return (
            <button
              key={kind}
              type="button"
              aria-label={label}
              title={label}
              style={{
                ...winButtonBaseStyle,
                background: isHovered ? (isClose ? TITLEBAR_CLOSE_HOVER_BG : SECONDARY_BG) : "transparent",
              }}
              onMouseEnter={() => setHover(kind)}
              onMouseLeave={() => setHover((h) => (h === kind ? null : h))}
              onClick={onClick}
            >
              <Icon size={12} />
            </button>
          );
        })}
      </div>
    </div>
  );
}
