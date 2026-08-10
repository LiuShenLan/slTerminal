// hooks 配置 IPC — 三层 hooks 子树读写封装（契约 C13-1，MC-212 泛化）
//
// 本文件是 agent_hooks_config_read / agent_hooks_config_write 两条命令的唯一 invoke 位置（硬约束 #1）。
// wrapper 加 cliId 首参（泛化命令全表见 MC-211；中间态由调用方传 CLAUDE_CLI_ID，Stage 06 hub 化时改 selectedCliId 回收）。
// 参数使用 camelCase（JS cliId/layer/projectPath ↔ Rust cli_id/layer/project_path 由 Tauri 自动转换）。
import { invoke } from "@tauri-apps/api/core";
import type { HooksLayer } from "../types/hooksConfig";

/**
 * 读取指定层的 hooks 子树（原始 JSON）
 *
 * 返回该层 settings.json 的 hooks 子树；文件不存在或无 hooks 键返回 null；
 * JSON 损坏时后端返回 Err（不返回 Null，防止损坏文件上编辑后 merge 丢字段）。
 * user 层不传 projectPath；project/local 层必须传（后端沙箱校验后拼接 .claude/settings.json）。
 */
export async function readHooksConfig(
  cliId: string,
  layer: HooksLayer,
  projectPath?: string,
): Promise<unknown> {
  return invoke("agent_hooks_config_read", {
    cliId,
    layer,
    ...(projectPath !== undefined ? { projectPath } : {}),
  });
}

/**
 * 写入指定层的 hooks 子树
 *
 * 后端 read-modify-write：读原文件 → 替换/插入 hooks 键 → 原子写，原样保留
 * permissions/env/$schema 等其他字段。hooks 必须为 JSON Object。
 */
export async function writeHooksConfig(
  cliId: string,
  layer: HooksLayer,
  hooks: unknown,
  projectPath?: string,
): Promise<void> {
  return invoke("agent_hooks_config_write", {
    cliId,
    layer,
    hooks,
    ...(projectPath !== undefined ? { projectPath } : {}),
  });
}
