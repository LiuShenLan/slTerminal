// PTY IPC 封装 — 前端调用 Pty 命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke, Channel } from "@tauri-apps/api/core";
import type { PtyEvent, SpawnRequest } from "../types/pty";

/**
 * 创建 PTY 会话并启动 shell
 *
 * 返回 sessionId；终端输出通过 onOutput Channel 异步推送。
 */
export async function spawn(
  request: SpawnRequest,
  onOutput: (event: PtyEvent) => void,
): Promise<string> {
  const channel = new Channel<PtyEvent>();
  channel.onmessage = onOutput;

  const sessionId: string = await invoke("pty_spawn", {
    request,
    onOutput: channel,
  });
  return sessionId;
}

/** 向 PTY 写入输入数据 */
export async function write(
  sessionId: string,
  data: Uint8Array,
): Promise<void> {
  await invoke("pty_write", {
    sessionId,
    data: Array.from(data),
  });
}

/** 调整 PTY 终端尺寸（fire-and-forget，不阻塞 UI） */
export async function resize(
  sessionId: string,
  cols: number,
  rows: number,
): Promise<void> {
  await invoke("pty_resize", { sessionId, cols, rows });
}

/** 销毁 PTY 会话 */
export async function kill(sessionId: string): Promise<void> {
  await invoke("pty_kill", { sessionId });
}
