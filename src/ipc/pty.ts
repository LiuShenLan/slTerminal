// PTY IPC 封装 — 前端调用 Pty 命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke, Channel } from "@tauri-apps/api/core";
import type { PtyEvent, SpawnRequest } from "../types/pty";

/** PTY 尺寸合法下界（ConPTY 最小 1 列/行） */
const MIN_PTY_DIM = 1;
/** PTY 尺寸合法上界（Rust SpawnRequest.cols/rows: u16 → 32767） */
const MAX_PTY_DIM = 32767;

/**
 * 校验 PTY 尺寸参数（FE-14 前置校验）：cols/rows 越界直接抛错，不 invoke。
 * 与后端 validate_spawn_request 的尺寸约束一致——前置拒绝避免非法参数进入 IPC。
 */
function assertPtyDim(cols: number, rows: number): void {
  if (!Number.isInteger(cols) || cols < MIN_PTY_DIM || cols > MAX_PTY_DIM) {
    throw new Error(
      `pty.spawn: cols 必须在 ${MIN_PTY_DIM}..=${MAX_PTY_DIM} 范围内（实际 ${cols}）`,
    );
  }
  if (!Number.isInteger(rows) || rows < MIN_PTY_DIM || rows > MAX_PTY_DIM) {
    throw new Error(
      `pty.spawn: rows 必须在 ${MIN_PTY_DIM}..=${MAX_PTY_DIM} 范围内（实际 ${rows}）`,
    );
  }
}

/**
 * 创建 PTY 会话并启动 shell
 *
 * 返回 sessionId；终端输出通过 onOutput Channel 异步推送。
 */
export async function spawn(
  request: SpawnRequest,
  onOutput: (event: PtyEvent) => void,
): Promise<string> {
  // FE-14：cols/rows 越界（含非整数/NaN）在 invoke 前拒绝，防止非法 ConPTY 创建参数
  assertPtyDim(request.cols, request.rows);

  const channel = new Channel<PtyEvent>();
  channel.onmessage = onOutput;

  const sessionId: string = await invoke("pty_spawn", {
    request,
    onOutput: channel,
  });
  return sessionId;
}

/**
 * 向 PTY 写入输入数据
 * @param sessionId PTY 会话 ID
 * @param panelId 面板 ID（归属校验，后端 SEC-08 校验）
 * @param data 输入字节
 */
export async function write(
  sessionId: string,
  panelId: string,
  data: Uint8Array,
): Promise<void> {
  await invoke("pty_write", {
    sessionId,
    panelId,
    data: Array.from(data),
  });
}

/**
 * 调整 PTY 终端尺寸（fire-and-forget，不阻塞 UI）
 * @param sessionId PTY 会话 ID
 * @param panelId 面板 ID（归属校验，后端 SEC-08 校验）
 * @param cols 列数
 * @param rows 行数
 */
export async function resize(
  sessionId: string,
  panelId: string,
  cols: number,
  rows: number,
): Promise<void> {
  await invoke("pty_resize", { sessionId, panelId, cols, rows });
}

/**
 * 销毁 PTY 会话
 * @param sessionId PTY 会话 ID
 * @param panelId 面板 ID（归属校验，后端 SEC-08 校验）
 */
export async function kill(sessionId: string, panelId: string): Promise<void> {
  await invoke("pty_kill", { sessionId, panelId });
}

/**
 * 获取 Windows 真实 build 号（F3 动态检测）
 *
 * 通过 RtlGetNtVersionNumbers 获取，取低 16 位。
 * 非 Windows 平台返回 21376（ConPTY 阈值 fallback）。
 */
export async function getWindowsBuildNumber(): Promise<number> {
  try {
    return await invoke<number>("get_windows_build_number");
  } catch {
    return 21376;
  }
}
