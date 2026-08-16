// sideViewDefs — 侧栏视图 side-effect 注册
//
// 此文件在 import 时执行副作用，向 sideViewRegistry 注册两条视图。
// 类比 cliProfiles/profiles 模式——side-effect import 触发注册。
// 新增侧栏视图只需在此追加 sideViewRegistry.register(...) 一行即可，
// 框架自动处理按钮渲染、开关、拖拽归属、槽位展示与持久化。
// 组件内不 import 本文件（防循环），由 Workspace 顶层引入。

import React from "react";
import { SidebarTree } from "../sidebar";
import { ExplorerPanel } from "../explorer";
import { CommitView } from "../commit";
import { AgentStatusView } from "../agentStatus/AgentStatusView";
import { sideViewRegistry } from "./sideViewRegistry";
import {
  IconNav,
  IconFiles,
  IconCommit,
  IconHistory,
} from "../../lib/icons";

// 注册项目列表视图（导航树）
sideViewRegistry.register({
  id: "projects",
  title: "项目列表",
  icon: IconNav,
  component: SidebarTree,
});

// 注册文件浏览器视图
// ExplorerPanel 不接受 props，箭头包装忽略 SideViewComponentProps
sideViewRegistry.register({
  id: "explorer",
  title: "文件浏览器",
  icon: IconFiles,
  component: () => React.createElement(ExplorerPanel),
});

// 注册 commit 视图
sideViewRegistry.register({
  id: "commit",
  title: "Commit",
  icon: IconCommit,
  component: () => React.createElement(CommitView),
});

// 注册 agent-status 视图（图标暂用时钟 IconHistory——含历史会话区；
// Stage 06 并入导航树后删除本视图）
sideViewRegistry.register({
  id: "agent-status",
  title: "Agent 状态",
  icon: IconHistory,
  component: AgentStatusView,
});
