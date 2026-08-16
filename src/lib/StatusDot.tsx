// StatusDot.tsx —— 会话/页签状态圆点（IC-02 / UI-504）
//
// props: { status: "working"|"attention"|"done"|"error", size?: number（默认 7）}
// 色映射（写死契约，F3 四态完整映射）：
//   working   → 绿  AGENT_STATUS_USAGE_COLORS.low（#86bb7a，运行中 = 用量低档绿）
//   attention → 黄  GIT_FILE_COLORS.modified（#d6b25e，等待/需注意 = 修改档黄）
//   done      → 灰  PLACEHOLDER_FG（#6b675f，完成/空闲 = 占位灰）
//   error     → 红  ERROR_FG（#d9706b，错误 = 错误前景红）
// 全部经 theme/colors.ts facade token 引用（硬约束 #6），禁止硬编码色值。
// 无描边/光晕/动画（纯静态圆点）。

import { AGENT_STATUS_USAGE_COLORS, ERROR_FG, GIT_FILE_COLORS, PLACEHOLDER_FG } from "../theme/colors";

/** 状态四态（与 agentStatus.ts 的 AgentStatus 同构，IC-03 渲染层统一改用本组件） */
export type StatusDotStatus = "working" | "attention" | "done" | "error";

const STATUS_COLORS: Record<StatusDotStatus, string> = {
  working: AGENT_STATUS_USAGE_COLORS.low,
  attention: GIT_FILE_COLORS.modified,
  done: PLACEHOLDER_FG,
  error: ERROR_FG,
};

export interface StatusDotProps {
  status: StatusDotStatus;
  /** 圆点直径，默认 7px */
  size?: number;
}

/** 状态圆点：纯装饰 div，无描边/光晕/动画 */
export function StatusDot({ status, size = 7 }: StatusDotProps) {
  return (
    <div
      aria-hidden="true"
      style={{
        width: size,
        height: size,
        borderRadius: "50%",
        backgroundColor: STATUS_COLORS[status],
      }}
    />
  );
}
