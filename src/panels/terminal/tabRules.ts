// tabRules — 命令行首 token→标题映射规则注册
//
// 此文件在 import 时执行副作用，向 tabTitleRegistry 注册规则。
// 注册的 command 键为首 token（命令行 trim().split(/\s+/)[0]）精确匹配键。
// 用户只需在此追加 tabTitleRegistry.register(...) 即可添加新的命令→标题映射。
// 不需要修改 useXterm.ts、TerminalPanel.tsx、DefaultTab、shell-integration.ps1 任一文件。

import { tabTitleRegistry } from "./TabTitleRegistry";

// 注册 claude code CLI 命令规则（首 token "claude" 匹配 claude / claude --resume / claude -p 等变体；无 icon——icon 由 P1-F3 hook 事件驱动设置）
tabTitleRegistry.register({ command: "claude", title: "claude" });
