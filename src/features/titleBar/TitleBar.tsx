// TitleBar —— 自绘窗口标题栏（TB-02 / UI-301）
//
// 定位：tauri.conf.json decorations:false 后由本组件承担原生标题栏职责——
// 拖拽（data-tauri-drag-region，TB-04）、双击最大化（TB-04）、
// 最小化/最大化/关闭三钮（经 src/ipc/window wrapper，契约见 TB-03）。
// 数据：中段标题 = projects store 活跃项目名 / 活跃页面名
//（layout store 的 activePageId 定位；无现成 selector，直接推导，不改 store）。

import { useState } from "react";
import type { CSSProperties, FC } from "react";
import type { IconProps } from "../../lib";
import { IconMin, IconMax, IconCloseWin } from "../../lib";
import { minimizeWindow, toggleMaximizeWindow, closeWindow } from "../../ipc/window";
import { useLayout } from "../../stores/layout";
import { useProjects, type Project } from "../../stores/projects";
import {
  TITLEBAR_BG, SEPARATOR_BG, DIM_FG, SECONDARY_BG, ACCENT_FG, ACTIVE_SELECTION_BG,
} from "../../theme/colors";

/** 关闭钮 hover 底——关闭危险色，设计定值（无 token 槽位，本组件内常量） */
const CLOSE_HOVER_BG = "#c04747";

/** app logo 终端提示符图形（lucide Terminal path，与设计稿 final-mockup 一致） */
const LOGO_PATH = "M5 8l6 5-6 5M13 19h7";

/** 窗口控制三钮定义（38×26、图标 12px，hover 底 ui.secondaryBg） */
type WinButtonKind = "min" | "max" | "close";
const WIN_BUTTONS: { kind: WinButtonKind; label: string; Icon: FC<IconProps>; onClick: () => void }[] = [
  { kind: "min", label: "最小化", Icon: IconMin, onClick: () => minimizeWindow() },
  { kind: "max", label: "最大化/还原", Icon: IconMax, onClick: () => toggleMaximizeWindow() },
  { kind: "close", label: "关闭", Icon: IconCloseWin, onClick: () => closeWindow() },
];

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
function useActiveProjectPage(): { project: Project | null; pageName: string } {
  const activePageId = useLayout((s) => s.activePageId);
  const projects = useProjects((s) => s.projects);
  const projList = Object.values(projects);
  if (projList.length === 0) return { project: null, pageName: "" };
  // 优先按全局活跃页面（layout store）定位所属项目
  if (activePageId) {
    for (const proj of projList) {
      const page = proj.pages.find((p) => p.pageId === activePageId);
      if (page) return { project: proj, pageName: page.name };
    }
  }
  // layout 无活跃页（未切换过页面/测试环境）：回退第一个项目的 activePageId 页
  const first = projList[0];
  const page = first.pages.find((p) => p.pageId === first.activePageId);
  if (page) return { project: first, pageName: page.name };
  return { project: null, pageName: "" };
}

export function TitleBar() {
  const { project, pageName } = useActiveProjectPage();
  const [hover, setHover] = useState<WinButtonKind | null>(null);

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
      {/* 左段：app 标识（拖拽区，TB-04） */}
      <div
        data-tauri-drag-region
        style={{ display: "flex", alignItems: "center", gap: 8, color: DIM_FG, fontWeight: 500 }}
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

      {/* 中段：活跃项目名 / 页面名（拖拽区 + 双击最大化，TB-04） */}
      <div
        data-tauri-drag-region
        style={{
          flex: 1,
          minWidth: 0,
          textAlign: "center",
          color: DIM_FG,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
        onDoubleClick={() => toggleMaximizeWindow()}
      >
        {project && (
          <>
            <span style={{ fontWeight: 500 }}>{project.name}</span>
            <span style={{ margin: "0 8px" }}>/</span>
            <span>{pageName}</span>
          </>
        )}
      </div>

      {/* 右段：窗口控制三钮（不在拖拽区内，保证可点击，TB-04） */}
      <div style={{ display: "flex", gap: 2 }}>
        {WIN_BUTTONS.map(({ kind, label, Icon, onClick }) => {
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
                background: isHovered ? (isClose ? CLOSE_HOVER_BG : SECONDARY_BG) : "transparent",
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
