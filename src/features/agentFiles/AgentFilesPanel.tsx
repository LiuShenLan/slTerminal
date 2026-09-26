// AgentFilesPanel.tsx — 「Agent 全局文件」侧栏视图（F1）
//
// 职责：agent 节点列表（注册序，仅声明 globalFiles 能力的 profile）→ 展开 =
// 该 agent 全局配置目录的文件浏览器（FileTreeExplorer 共享组件，功能与文件浏览器相同）。
//
// 关键语义：
// - 展示内容可配置（store agentGlobalFiles 段）：全部/自定义模式 + 「显示运行时文件」开关
//   → 派生 rootFilter/eventPathFilter 下传 FileTreeExplorer（配置变更经引用变化即时生效）
// - 展开即启动 pinned watcher（ADR-0024：不参与 pause_all_except，与项目 watcher 互不干扰）；
//   折叠/卸载即停止；目录不存在（exists=false）不启动监听，展开显示「目录不存在」占位
// - 双击打开：无活跃页面/无宿主 → toast「请先创建项目」；否则 openFileInPage
//   rootPath/projectRootPath 传 null（agent 文件在项目外，标题 = basename）
// - 跨挂载状态（CP-016 槽位）：viewState = { expandedAgents, trees: Record<cliId, FileTreeViewState> }
//   ——agent 展开集 + 每棵树展开态均经注册表状态槽存活，槽位切换/换区重建不丢

import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { cliProfileRegistry } from "../cliProfiles/cliProfileRegistry";
import type { CodingCliProfile } from "../cliProfiles/types";
import type { AgentGlobalDir } from "../../types/agentDirs";
import { listAgentDirs } from "../../ipc/agentDirs";
import { startWatch, stopWatch } from "../../ipc/notify";
import { useAgentGlobalFiles } from "../../stores/agentGlobalFiles";
import { useLayout } from "../../stores/layout";
import {
  defaultConfig,
  isRootEventRelevant,
  shouldShowAtRoot,
} from "./filtering";
import { FileTreeExplorer } from "../explorer/FileTreeExplorer";
import type { FileTreeViewState } from "../explorer/useFileTree";
import { openFileInPage } from "../../workspace/openFile";
import { toast } from "../../lib";
import {
  EXPLORER_COLORS,
  SEPARATOR_BG,
  DIM_FG,
  PLACEHOLDER_FG,
} from "../../theme";
import { IconChevronDown, IconChevronRight, IconEmptyBox } from "../../lib/icons";
import type { SideViewComponentProps } from "../sideViews/sideViewRegistry";

/** 视图跨挂载状态槽形态（CP-016） */
interface AgentFilesViewState {
  /** 已展开的 agent cliId 集合 */
  expandedAgents: string[];
  /** 每棵树的展开态快照（FileTreeExplorer 槽位透传） */
  trees: Record<string, FileTreeViewState>;
}

/** 单 agent 区块（节点行 + 展开树）——独立组件承载每 agent 的 watch 生命周期与过滤派生 */
const AgentSection: React.FC<{
  profile: CodingCliProfile;
  dir: AgentGlobalDir | undefined;
  expanded: boolean;
  onToggle: () => void;
  treeViewState: FileTreeViewState | undefined;
  onTreeViewStateChange: (state: FileTreeViewState) => void;
}> = ({ profile, dir, expanded, onToggle, treeViewState, onTreeViewStateChange }) => {
  const [hovered, setHovered] = useState(false);
  const activePageId = useLayout((s) => s.activePageId);

  const globalFiles = profile.capabilities.globalFiles;
  const runtimePaths = useMemo(
    () => globalFiles?.runtimePaths ?? [],
    [globalFiles],
  );

  // store 缺该 cliId 配置时回退缺省——useMemo 稳定引用（defaultConfig() 每次新对象，
  // 直用会令 rootFilter 每渲染换引用 → 触发 useFileTree「过滤器变化即刷新」effect 死循环）
  const cfgRaw = useAgentGlobalFiles((s) => s.configs[profile.id]);
  const config = useMemo(() => cfgRaw ?? defaultConfig(), [cfgRaw]);

  const rootFilter = useCallback(
    (name: string) => shouldShowAtRoot(name, config, runtimePaths),
    [config, runtimePaths],
  );
  const dirPath = dir?.path ?? null;
  const eventPathFilter = useCallback(
    (absPath: string) =>
      dirPath !== null &&
      isRootEventRelevant(absPath, dirPath, config, runtimePaths),
    [dirPath, config, runtimePaths],
  );

  // pinned watcher 生命周期（ADR-0024）：展开且目录存在 → 启动；折叠/卸载 → 停止
  useEffect(() => {
    if (!expanded || !dir?.exists) return;
    void startWatch(dir.path, { pinned: true });
    return () => {
      void stopWatch(dir.path);
    };
  }, [expanded, dir?.exists, dir?.path]);

  /** 双击打开：无活跃页面/无宿主 → toast；否则项目外形态打开（标题 = basename） */
  const handleOpenFile = useCallback(
    (filePath: string) => {
      const dockApi = window.__dockviewApi;
      if (!activePageId || !dockApi) {
        toast.show("warning", "请先创建项目");
        return;
      }
      openFileInPage(
        { activePageId, dockApi, rootPath: null, projectRootPath: null },
        filePath,
      );
    },
    [activePageId],
  );

  return (
    <div>
      {/* agent 节点行：展开箭头 + CLI logo + displayName */}
      <div
        data-e2e={`agent-files-node-${profile.id}`}
        onClick={onToggle}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 4,
          padding: "3px 6px",
          cursor: "pointer",
          userSelect: "none",
          fontSize: 12,
          color: EXPLORER_COLORS.fg,
          background: hovered ? EXPLORER_COLORS.hover : "transparent",
        }}
      >
        <span style={{ display: "flex", color: EXPLORER_COLORS.arrowClosed }}>
          {expanded ? (
            <IconChevronDown size={12} />
          ) : (
            <IconChevronRight size={12} />
          )}
        </span>
        <img
          src={profile.iconSrc}
          width={14}
          height={14}
          style={{ flexShrink: 0 }}
          alt=""
        />
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {profile.displayName}
        </span>
      </div>

      {expanded && (
        <div style={{ paddingLeft: 10 }}>
          {dir?.exists ? (
            <FileTreeExplorer
              rootPath={dir.path}
              projectRootPath={null}
              viewState={treeViewState}
              onViewStateChange={(s) =>
                onTreeViewStateChange(s as FileTreeViewState)
              }
              rootFilter={rootFilter}
              eventPathFilter={eventPathFilter}
              onOpenFile={handleOpenFile}
              testIdPrefix={`agent-files-${profile.id}`}
            />
          ) : (
            // 目录不存在（未运行过该 agent / home 下无配置目录）→ 占位，不启动监听
            <div
              data-e2e={`agent-files-missing-${profile.id}`}
              style={{
                padding: "8px 6px",
                fontSize: 12,
                color: DIM_FG,
                userSelect: "none",
              }}
            >
              目录不存在
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export const AgentFilesPanel: React.FC<SideViewComponentProps> = ({
  viewState,
  onViewStateChange,
}) => {
  // 挂载消费一次槽位快照（viewState 不入依赖——与 useFileTree 同语义）
  const initialRef = useRef(viewState as AgentFilesViewState | undefined);
  const [expandedAgents, setExpandedAgents] = useState<string[]>(
    initialRef.current?.expandedAgents ?? [],
  );
  const treesRef = useRef<Record<string, FileTreeViewState>>(
    initialRef.current?.trees ?? {},
  );
  const expandedRef = useRef(expandedAgents);
  const onViewStateChangeRef = useRef(onViewStateChange);
  onViewStateChangeRef.current = onViewStateChange;

  const [dirs, setDirs] = useState<AgentGlobalDir[] | null>(null);
  useEffect(() => {
    let cancelled = false;
    listAgentDirs()
      .then((list) => {
        if (!cancelled) setDirs(list);
      })
      .catch((err) => {
        console.error("[slTerminal] agent_dirs_list 失败:", err);
        if (!cancelled) setDirs([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 仅声明 globalFiles 能力的 profile 参与（注册序）
  const agents = useMemo(
    () => cliProfileRegistry.getAll().filter((p) => p.capabilities.globalFiles),
    [],
  );

  const toggleAgent = useCallback((cliId: string) => {
    const cur = expandedRef.current;
    const next = cur.includes(cliId)
      ? cur.filter((id) => id !== cliId)
      : [...cur, cliId];
    expandedRef.current = next;
    setExpandedAgents(next);
    onViewStateChangeRef.current?.({
      expandedAgents: next,
      trees: treesRef.current,
    } satisfies AgentFilesViewState);
  }, []);

  /** 子树展开态上呼：合入 trees 槽位后整份上呼 */
  const handleTreeViewStateChange = useCallback(
    (cliId: string, state: FileTreeViewState) => {
      treesRef.current = { ...treesRef.current, [cliId]: state };
      onViewStateChangeRef.current?.({
        expandedAgents: expandedRef.current,
        trees: treesRef.current,
      } satisfies AgentFilesViewState);
    },
    [],
  );

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        background: EXPLORER_COLORS.bg,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
      data-e2e="agent-files-panel"
    >
      {/* 标题栏（样式照 ExplorerPanel） */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          padding: "4px 8px",
          borderBottom: `1px solid ${SEPARATOR_BG}`,
          height: 28,
          fontSize: 11,
          color: DIM_FG,
          textTransform: "uppercase",
          letterSpacing: "0.08em",
          userSelect: "none",
          flexShrink: 0,
        }}
      >
        Agent 全局文件
      </div>

      <div style={{ flex: 1, overflowY: "auto", overflowX: "hidden", padding: "2px 0" }}>
        {dirs === null ? (
          <div style={{ padding: 16, fontSize: 12, color: DIM_FG, textAlign: "center", userSelect: "none" }}>
            加载中…
          </div>
        ) : agents.length === 0 ? (
          // 无声明 globalFiles 能力的 agent（防御——当前实现必有 claude）
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 8,
              padding: 16,
              fontSize: 12,
              color: DIM_FG,
              textAlign: "center",
              userSelect: "none",
            }}
          >
            <span style={{ color: PLACEHOLDER_FG, display: "flex" }}>
              <IconEmptyBox size={15} />
            </span>
            <span>无可用 Agent</span>
          </div>
        ) : (
          agents.map((profile) => (
            <AgentSection
              key={profile.id}
              profile={profile}
              dir={dirs.find((d) => d.cliId === profile.id)}
              expanded={expandedAgents.includes(profile.id)}
              onToggle={() => toggleAgent(profile.id)}
              treeViewState={treesRef.current[profile.id]}
              onTreeViewStateChange={(s) =>
                handleTreeViewStateChange(profile.id, s)
              }
            />
          ))
        )}
      </div>
    </div>
  );
};
