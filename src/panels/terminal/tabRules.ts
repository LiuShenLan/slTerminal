// tabRules — 命令行→标题/图标规则注册
//
// 此文件在 import 时执行副作用，向 tabTitleRegistry 注册规则。
// 用户只需在此追加 tabTitleRegistry.register(...) 即可添加新的命令→标题映射。
// 不需要修改 useXterm.ts、TerminalPanel.tsx、DefaultTab、shell-integration.ps1 任一文件。

import { tabTitleRegistry } from "./TabTitleRegistry";
// Vite 处理为 hashed 文件名或 data URL
import claudeLogo from "../../assets/claude-logo.png";

// 注册 claude code CLI 命令规则
tabTitleRegistry.register({ command: "claude", title: "claude", icon: claudeLogo });
