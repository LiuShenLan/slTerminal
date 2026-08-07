// schemes/index — 配色方案 side-effect 注册
//
// 此文件在 import 时执行副作用，向 schemeRegistry 注册内置方案。
// 照 src/panels/terminal/tabRules.ts / src/features/sideViews/sideViewDefs.ts 模式——
// side-effect import 触发注册（main.tsx 启动序列在 setActive 前 await import 本文件）。
// 新增方案只需在此追加 import + schemeRegistry.register(...) 一行，
// 消费方（colors.ts facade / overrides.ts）无需任何改动。

import { schemeRegistry } from "../schemeRegistry";
import { darcula } from "./darcula";

// 注册内置默认方案（id "darcula" 即 settings.json colorScheme 段缺省取值）
schemeRegistry.register(darcula);
