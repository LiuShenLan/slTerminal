// profiles/index.ts — CLI profile 注册触发点（side-effect import）
//
// 照 tabRules side-effect 先例：import 本文件即执行全部 profile 注册。
// 新增 CLI 在此追加 import "./<cli>"，不修改核心逻辑。
// 生产注册触发点：Workspace.tsx import 本文件（D-07）。

import "./claude";
