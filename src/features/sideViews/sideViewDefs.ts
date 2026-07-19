// sideViewDefs — 侧栏视图 side-effect 注册
//
// 此文件在 import 时执行副作用，向 sideViewRegistry 注册两条视图。
// 类比 src/panels/terminal/tabRules.ts 模式——side-effect import 触发注册。
// 新增侧栏视图只需在此追加 sideViewRegistry.register(...) 一行即可，
// 框架自动处理按钮渲染、开关、拖拽归属、槽位展示与持久化。
// 组件内不 import 本文件（防循环），由 Workspace 顶层引入。

import React from "react";
import { SidebarTree } from "../sidebar";
import { ExplorerPanel } from "../explorer";
import { sideViewRegistry } from "./sideViewRegistry";

// 注册项目列表视图（📋）
sideViewRegistry.register({
  id: "projects",
  title: "项目列表",
  icon: "📋",
  component: SidebarTree,
});

// 注册文件浏览器视图（📁）
// ExplorerPanel 不接受 props，箭头包装忽略 SideViewComponentProps
sideViewRegistry.register({
  id: "explorer",
  title: "文件浏览器",
  icon: "📁",
  component: () => React.createElement(ExplorerPanel),
});
