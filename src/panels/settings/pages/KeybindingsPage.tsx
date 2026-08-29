// KeybindingsPage — 设置中心「快捷键」配置页（F11，SC-FE-09）
//
// 读取：listCommands() 枚举注册表当前命令 + useKeybindings 覆盖层（user store）。
//   生效键显示经 getEffectiveKeystroke(id)（默认 ⊕ 覆盖合并，与运行期同源），
//   防显示/运行漂移；hasOwnProperty(overrides, id) 判定是否有用户覆盖（高亮 + ↺ 复位）。
// 录制：行点击进入录制态 → setCaptureSuspended(true) 屏蔽全局快捷键（防录
//   Ctrl+Shift+C 时真执行复制）；window keydown capture 监听：
//   isComposing 跳过 / Escape 取消 / Backspace|Delete → setBinding(id, null) 解绑 /
//   纯修饰键（code 为 Control*/Shift*/Alt*/Meta*）忽略 / isReserved → 行内红字拒绝
//   （保持录制等下一键）/ findConflict（同 context 他命令生效键相同）→ 警告
//   「与 XX 冲突，生效按优先级派发」但允许写入 / 合法 → setBinding(id, formatKeystroke(ks))。
//   结束/取消/卸载均经录制 effect 的 cleanup 兜底 setCaptureSuspended(false)。
// 写入：经 useKeybindings.setBinding/clearBinding（loaded 守卫 + 2s debounce 落盘
//   settings.json keybindings 段，wireKeybindings 同步注册表）。

import React, { useEffect, useMemo, useState } from "react";
import { getShortcutRegistry } from "../../../features/shortcuts/ShortcutRegistry";
import { useKeybindings } from "../../../stores/keybindings";
import { isReserved } from "../../../features/shortcuts/reserved";
import { formatKeystroke } from "../../../features/shortcuts/keystroke";
import type {
  CommandCategory,
  CommandMeta,
  KeyStroke,
} from "../../../features/shortcuts/types";
import {
  PANEL_BG,
  SIDEBAR_FG,
  DIM_FG,
  PLACEHOLDER_FG,
  ACCENT_FG,
  ERROR_FG,
  ACTIVE_SELECTION_BG,
  BUTTON_FG,
  INPUT_BG,
  INPUT_BORDER,
} from "../../../theme";
import type { SettingsPageProps } from "../../../features/settingsCenter/types";

/** 分组展示序（目录序）：global/terminal/editor/explorer */
const CATEGORY_ORDER: CommandCategory[] = ["global", "terminal", "editor", "explorer"];

/** 分组标题 */
const CATEGORY_TITLES: Record<CommandCategory, string> = {
  global: "全局",
  terminal: "终端",
  editor: "编辑器",
  explorer: "文件浏览器",
};

/**
 * 查找同 context 下与 keystroke 生效键冲突的其他命令（不含自身）。
 * 页内纯函数（单测）：绑定前提示「冲突但放行」——同 context 键位冲突
 * 由运行期优先级/焦点栈派发决定谁生效（findWinner winner-take-all）。
 * @returns 首个冲突命令；无冲突 / 自身 / 未知 id → null
 */
export function findConflict(
  commands: readonly CommandMeta[],
  getEffective: (id: string) => string | null,
  id: string,
  keystroke: KeyStroke,
): CommandMeta | null {
  const target = commands.find((c) => c.id === id);
  if (!target) return null;
  const fp = formatKeystroke(keystroke);
  return (
    commands.find(
      (c) => c.id !== id && c.context === target.context && getEffective(c.id) === fp,
    ) ?? null
  );
}

const KeybindingsPage: React.FC<SettingsPageProps> = () => {
  const registry = getShortcutRegistry();
  const overrides = useKeybindings((s) => s.overrides);
  const setBinding = useKeybindings((s) => s.setBinding);
  const clearBinding = useKeybindings((s) => s.clearBinding);

  /** 当前已注册命令元数据（listCommands，注册表真值源） */
  const commands = useMemo(() => registry.listCommands(), [registry]);

  /** 录制中的命令 id（null = 非录制态） */
  const [recordingId, setRecordingId] = useState<string | null>(null);
  /** 行内错误（保留键拒绝）——仅录制中显示，保持录制等下一键 */
  const [rowError, setRowError] = useState<string | null>(null);
  /** 行内冲突警告——写入后仍显示（放行语义），下次录制开始清除 */
  const [rowWarning, setRowWarning] = useState<{ id: string; message: string } | null>(null);

  /** 按 category 分组（目录序） */
  const grouped = useMemo(() => {
    const map = new Map<CommandCategory, CommandMeta[]>();
    for (const cat of CATEGORY_ORDER) map.set(cat, []);
    for (const c of commands) map.get(c.category)?.push(c);
    return map;
  }, [commands]);

  // 录制态 effect：挂 window keydown capture 监听 + 屏蔽全局快捷键；
  // cleanup 统一收口（结束/取消/卸载）→ setCaptureSuspended(false) 兜底。
  useEffect(() => {
    if (recordingId === null) return;
    registry.setCaptureSuspended(true);
    const target = commands.find((c) => c.id === recordingId);

    const handler = (e: KeyboardEvent) => {
      // IME 组合态跳过（中文输入法合成中不视为录制按键）
      if (e.isComposing) return;
      // Escape 取消录制
      if (e.code === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        setRecordingId(null);
        return;
      }
      // Backspace | Delete → 解绑
      if (e.code === "Backspace" || e.code === "Delete") {
        e.preventDefault();
        e.stopPropagation();
        setBinding(recordingId, null);
        setRecordingId(null);
        return;
      }
      // 纯修饰键忽略（code 为 ControlLeft/ShiftRight/AltLeft/MetaLeft 等）
      if (/^(Control|Shift|Alt|Meta)/.test(e.code)) return;
      if (!e.code || !target) return;
      e.preventDefault();
      e.stopPropagation();

      const ks: KeyStroke = {
        ctrlKey: e.ctrlKey,
        shiftKey: e.shiftKey,
        altKey: e.altKey,
        metaKey: e.metaKey,
        code: e.code,
      };
      // 保留键（终端控制字符/编辑器内部键）→ 行内红字拒绝，保持录制等下一键
      if (isReserved(ks, target.context)) {
        setRowWarning(null);
        setRowError(`保留键 ${formatKeystroke(ks)} 不可绑定`);
        return;
      }
      // 同 context 冲突 → 警告但允许写入（运行期按优先级/焦点派发）
      const conflict = findConflict(
        commands,
        (id) => registry.getEffectiveKeystroke(id),
        recordingId,
        ks,
      );
      if (conflict) {
        setRowWarning({
          id: recordingId,
          message: `与「${conflict.title}」冲突，生效按优先级派发`,
        });
      } else {
        setRowWarning(null);
      }
      setRowError(null);
      setBinding(recordingId, formatKeystroke(ks));
      setRecordingId(null);
    };

    window.addEventListener("keydown", handler, { capture: true });
    return () => {
      window.removeEventListener("keydown", handler, { capture: true });
      registry.setCaptureSuspended(false); // 结束/取消/卸载兜底
    };
  }, [recordingId, commands, registry, setBinding]);

  /** 进入录制态：清行内提示 + 置 recordingId（effect 挂监听 + 屏蔽） */
  const startRecording = (id: string) => {
    setRowError(null);
    setRowWarning(null);
    setRecordingId(id);
  };

  /** ↺ 复位：清覆盖回退默认键（stopPropagation 防误触录制） */
  const handleReset = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    clearBinding(id);
  };

  return (
    <div
      data-e2e="settings-keybindings-page"
      style={{ width: "100%", height: "100%", background: PANEL_BG, overflowY: "auto" }}
    >
      <div style={{ padding: "16px 20px" }}>
        {CATEGORY_ORDER.map((cat) => {
          const list = grouped.get(cat) ?? [];
          if (list.length === 0) return null;
          return (
            <div key={cat} style={{ marginBottom: 16 }}>
              <div
                data-e2e={`kb-group-${cat}`}
                style={{ fontSize: 12, color: DIM_FG, marginBottom: 6 }}
              >
                {CATEGORY_TITLES[cat]}
              </div>
              {list.map((cmd) => {
                const effective = registry.getEffectiveKeystroke(cmd.id);
                const hasOverride = Object.prototype.hasOwnProperty.call(overrides, cmd.id);
                const defaultStr = cmd.defaultKey ? formatKeystroke(cmd.defaultKey) : null;
                const recording = recordingId === cmd.id;
                return (
                  <div key={cmd.id}>
                    <div
                      data-e2e={`kb-row-${cmd.id}`}
                      onClick={() => startRecording(cmd.id)}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        gap: 12,
                        padding: "8px 10px",
                        borderRadius: 4,
                        cursor: "pointer",
                        background: recording ? ACTIVE_SELECTION_BG : undefined,
                      }}
                    >
                      <span style={{ fontSize: 13, color: SIDEBAR_FG }}>{cmd.title}</span>
                      {recording ? (
                        <span data-e2e="kb-recording-hint" style={{ fontSize: 12, color: DIM_FG }}>
                          按下新键位…Esc 取消
                        </span>
                      ) : effective === null ? (
                        <span data-e2e="kb-unbound" style={{ fontSize: 12, color: PLACEHOLDER_FG }}>
                          未绑定
                        </span>
                      ) : (
                        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                          <span
                            data-e2e="kb-effective"
                            style={{ fontSize: 13, color: hasOverride ? ACCENT_FG : SIDEBAR_FG }}
                          >
                            {effective}
                          </span>
                          {hasOverride && (
                            <button
                              data-e2e="kb-reset"
                              title="恢复默认键"
                              onClick={(e) => handleReset(e, cmd.id)}
                              style={{
                                fontSize: 12,
                                lineHeight: 1,
                                padding: "2px 6px",
                                color: BUTTON_FG,
                                background: INPUT_BG,
                                border: `1px solid ${INPUT_BORDER}`,
                                borderRadius: 3,
                                cursor: "pointer",
                              }}
                            >
                              ↺
                            </button>
                          )}
                          {hasOverride && defaultStr && (
                            <span data-e2e="kb-default" style={{ fontSize: 11, color: DIM_FG }}>
                              {defaultStr}
                            </span>
                          )}
                        </span>
                      )}
                    </div>
                    {recording && rowError && (
                      <div
                        data-e2e={`kb-error-${cmd.id}`}
                        style={{ padding: "2px 10px 6px", fontSize: 12, color: ERROR_FG }}
                      >
                        {rowError}
                      </div>
                    )}
                    {rowWarning && rowWarning.id === cmd.id && (
                      <div
                        data-e2e={`kb-warning-${cmd.id}`}
                        style={{ padding: "2px 10px 6px", fontSize: 12, color: ACCENT_FG }}
                      >
                        {rowWarning.message}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default KeybindingsPage;
