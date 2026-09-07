// ConptyInputModesPage — 设置中心「终端输入模式」配置页（CP-009，global 组）
//
// 读取：store（useConptyInputModes）在 App 启动链 loadFromDisk；开关行纯订阅渲染。
// 提交（立即提交型，无 dirty 暂存，照 BackgroundTasksPage 先例）：
//   切换即 setMode → store 变更 → 2s debounce 经 save_settings 写 conptyInputModes 段
//   （后端白名单第 7 键），失败经 store 内统一 toast（FE-09）。
// 生效闭环：新终端 spawn 时后端读段计算 ConPTY flags——已开终端不受即时影响，
//   重启终端（或重开面板）后生效。
// passthrough 开关旁**常驻**警示（ADR-0007 门禁第 3 条）：0x8 下 claude 等全屏
//   TUI 鼠标滚轮失效为真实 claude 场景独有复现（自动化不可守卫），启用须实测验证。

import React from "react";
import { useConptyInputModes } from "../../../stores/conptyInputModes";
import { PANEL_BG, SIDEBAR_FG, DIM_FG, ERROR_FG, SEPARATOR_BG } from "../../../theme";
import type { SettingsPageProps } from "../../../features/settingsCenter/types";

/** 开关行数据（flag 位注释与后端 ConptyInputModes 字段一一对应，双源漂移防复发） */
const MODE_ROWS: Array<{
  key: "inheritCursor" | "resizeQuirk" | "win32InputMode" | "passthroughMode";
  dataE2e: string;
  label: string;
  bit: string;
  description: string;
}> = [
  {
    key: "inheritCursor",
    dataE2e: "settings-conpty-inherit-cursor",
    label: "继承光标位置",
    bit: "0x1",
    description: "INHERIT_CURSOR——子进程启动时继承终端光标状态",
  },
  {
    key: "resizeQuirk",
    dataE2e: "settings-conpty-resize-quirk",
    label: "Resize 兼容",
    bit: "0x2",
    description: "RESIZE_QUIRK——创建时应用窗口尺寸，避免启动首帧错位",
  },
  {
    key: "win32InputMode",
    dataE2e: "settings-conpty-win32-input-mode",
    label: "Win32 输入模式",
    bit: "0x4",
    description:
      "WIN32_INPUT_MODE——发送 Win32 键事件而非 VT 序列；系统 conhost 的 Win10（build < 21376）回退路径不生效",
  },
  {
    key: "passthroughMode",
    dataE2e: "settings-conpty-passthrough-mode",
    label: "Passthrough 直通模式",
    bit: "0x8",
    description:
      "PASSTHROUGH_MODE——子进程输出原样透传，conhost 不做 VT 解析（默认关闭，须实测验证后启用）",
  },
];

const ConptyInputModesPage: React.FC<SettingsPageProps> = () => {
  const modes = useConptyInputModes((s) => s.modes);
  const setMode = useConptyInputModes((s) => s.setMode);

  return (
    <div
      style={{ width: "100%", height: "100%", background: PANEL_BG, overflowY: "auto" }}
      data-e2e="settings-conpty-input-modes-page"
    >
      <div style={{ padding: "16px 20px" }}>
        {/* 页首说明：矩阵含义 + 生效时机 */}
        <div style={{ fontSize: 12, color: DIM_FG, marginBottom: 16 }}>
          控制新建终端会话的 ConPTY 输入模式位组合（影响已开终端需重开生效）；默认值
          与应用内置行为一致，改动前请确认兼容性。
        </div>
        {MODE_ROWS.map((row) => {
          const inputId = `settings-conpty-${row.key}`;
          return (
            <div key={row.key} style={{ marginBottom: 18 }} data-e2e={row.dataE2e}>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  fontSize: 13,
                  color: SIDEBAR_FG,
                }}
              >
                <input
                  id={inputId}
                  type="checkbox"
                  checked={modes[row.key]}
                  data-e2e={`${row.dataE2e}-checked`}
                  onChange={(e) => setMode({ [row.key]: e.target.checked })}
                />
                <label htmlFor={inputId}>
                  {row.label}（{row.bit}）
                </label>
              </div>
              <div style={{ marginTop: 4, marginLeft: 24, fontSize: 12, color: DIM_FG }}>
                {row.description}
              </div>
              {row.key === "passthroughMode" && (
                /* CP-009：passthrough 常驻警示（ADR-0007 门禁第 3 条口径——
                   0x8 滚轮失效仅真实 claude 场景复现，自动化不可守卫） */
                <div
                  data-e2e="settings-conpty-passthrough-warning"
                  style={{
                    marginTop: 8,
                    marginLeft: 24,
                    padding: "8px 10px",
                    fontSize: 12,
                    color: ERROR_FG,
                    border: `1px solid ${SEPARATOR_BG}`,
                    borderRadius: 4,
                  }}
                >
                  警示：启用后将导致 claude 等全屏 TUI 鼠标滚轮失效，变更须实测验证
                  （真实 claude 全屏滚轮测试）后方可保留。
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default ConptyInputModesPage;
