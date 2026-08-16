// sideViewDefs — 侧栏视图 side-effect 注册
//
// 此文件在 import 时执行副作用，向 sideViewRegistry 注册三条视图（NAV-05 三槽）。
// 类比 cliProfiles/profiles 模式——side-effect import 触发注册。
// 新增侧栏视图只需在此追加 sideViewRegistry.register(...) 一行即可，
// 框架自动处理按钮渲染、开关、拖拽归属、槽位展示与持久化。
// 组件内不 import 本文件（防循环），由 Workspace 顶层引入。
//
// 底部「配置」钮（NAV-05）：不入本注册表——不参与拖拽/持久化，
// 由 ActivityBar 底部固定渲染（id config，点击 = 打开 hooksConfig 面板）。

import React from "react";
import { NavTree } from "../navTree";
import { ExplorerPanel } from "../explorer";
import { CommitView } from "../commit";
import { sideViewRegistry } from "./sideViewRegistry";
import {
  IconNav,
  IconFiles,
  IconCommit,
} from "../../lib/icons";

// 注册导航树视图（NAV-05：原 projects/agent-status 两视图并入导航树）
sideViewRegistry.register({
  id: "nav",
  title: "导航树",
  icon: IconNav,
  component: NavTree,
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
