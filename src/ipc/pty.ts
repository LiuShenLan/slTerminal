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

/** PTY 重连 — 替换 Channel 并回放 ring buffer（E1）
 *
 * 页面切换后调用，将新 Channel 绑定到已有 session，
 * 回放 ring buffer 中缓存的最近输出以恢复终端内容。
 */
export async function reattach(
  sessionId: string,
  onOutput: (event: PtyEvent) => void,
): Promise<void> {
  const channel = new Channel<PtyEvent>();
  channel.onmessage = onOutput;
  await invoke("pty_reattach", {
    sessionId,
    onOutput: channel,
  });
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
