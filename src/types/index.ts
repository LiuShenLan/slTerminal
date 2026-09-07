// DTO 单源化(CP-024):本目录 9 个域文件由 ts-rs 从 Rust derive 生成
// (Rust 侧 #[ts(export_to = "../../src/types/<域>.ts")],ts-rs 运行期以
// bindings/ 为 out_dir,../../ 抵消之),禁手改——改 DTO 走 Rust + 重跑
// cargo test export_bindings。残面见 local.ts / hooksConfigGui.ts。
export * from "./pty";
export * from "./fs";
export * from "./git";
export * from "./notify";
export * from "./agent";
export * from "./agentHistory";
export * from "./hooksConfig";
export * from "./backgroundTasks";
export * from "./planBalance";
export * from "./local";
export type { HooksConfigGui, HookEventGroup, HookMatcherGroup, HookHandlerGui } from "./hooksConfigGui";
