// BackgroundTasksPage — 设置中心「后台定时任务」配置页（F12）
//
// 读取：挂载经 background_tasks_list() 拿任务清单 + 生效配置（通用行组件纯渲染——
//       新增任务自动出现，页零改动）。
// 提交（立即提交型，无 dirty 暂存，照 F11 设置页先例）：
//   勾选切换立即提交 set_config(taskId, {enabled})；
//   频率失焦/回车提交，非法（非数/非整数/越界）→ 行内红字提示，不提交不 toast；
//   后端拒绝 → toast + 保留用户输入。
// 生效闭环：set_config 返回完整清单 → 更新行；sessionRefresh 直调调度器 applyConfig
//   即时生效；planBalance 由后端内存值即时生效（footer 经 background-tasks-updated
//   事件感知），并调一次 refreshPlanBalance() 拉取最新余量（照 F11 反馈闭环先例）。

import React, { useCallback, useEffect, useState } from "react";
import {
  listBackgroundTasks,
  setBackgroundTaskConfig,
} from "../../../ipc/backgroundTasks";
import { refreshPlanBalance } from "../../../ipc/planBalance";
import {
  PLAN_BALANCE_TASK_ID,
  SESSION_REFRESH_TASK_ID,
  type BackgroundTaskInfo,
} from "../../../types/backgroundTasks";
import { backgroundTaskScheduler } from "../../../features/backgroundTasks";
import "../../../features/backgroundTasks/tasks"; // side-effect：applyConfig 目标注册保障
import { toast, getErrorMessage } from "../../../lib";
import {
  PANEL_BG,
  SIDEBAR_FG,
  DIM_FG,
  INPUT_BG,
  INPUT_BORDER,
  FOCUS_BORDER,
  ERROR_FG,
} from "../../../theme";
import type { SettingsPageProps } from "../../../features/settingsCenter/types";

/** 行内非法提示（范围提示——DTO 无 default 字段，不写默认值） */
function rangeHint(task: BackgroundTaskInfo): string {
  return `${task.intervalMin}–${task.intervalMax} 秒`;
}

/** 通用任务行（纯渲染——新增任务自动出现） */
function TaskRow(props: {
  task: BackgroundTaskInfo;
  input: string;
  error: string | null;
  onToggle(enabled: boolean): void;
  onInput(v: string): void;
  onCommitInterval(): void;
}) {
  const { task, input, error, onToggle, onInput, onCommitInterval } = props;
  const inputId = `settings-background-tasks-interval-${task.taskId}`;
  return (
    <div data-e2e={`settings-background-tasks-row-${task.taskId}`} style={{ marginBottom: 16 }}>
      <div
        style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: SIDEBAR_FG }}
      >
        <input
          type="checkbox"
          checked={task.enabled}
          data-e2e={`settings-background-tasks-enabled-${task.taskId}`}
          onChange={(e) => onToggle(e.target.checked)}
        />
        <label htmlFor={inputId}>{task.title}频率（秒）</label>
        <input
          id={inputId}
          type="text"
          inputMode="numeric"
          value={input}
          data-e2e={`settings-background-tasks-interval-${task.taskId}`}
          onChange={(e) => onInput(e.target.value)}
          onFocus={(e) => {
            e.currentTarget.style.borderColor = FOCUS_BORDER;
          }}
          onBlur={(e) => {
            e.currentTarget.style.borderColor = INPUT_BORDER;
            onCommitInterval();
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") onCommitInterval();
          }}
          style={{
            width: 90,
            padding: "4px 8px",
            fontSize: 13,
            color: SIDEBAR_FG,
            background: INPUT_BG,
            border: `1px solid ${INPUT_BORDER}`,
            borderRadius: 4,
            outline: "none",
          }}
        />
        <span style={{ fontSize: 12, color: DIM_FG }}>{rangeHint(task)}</span>
      </div>
      {error && (
        <div
          data-e2e={`settings-background-tasks-error-${task.taskId}`}
          style={{ marginTop: 8, fontSize: 12, color: ERROR_FG }}
        >
          {error}
        </div>
      )}
    </div>
  );
}

const BackgroundTasksPage: React.FC<SettingsPageProps> = () => {
  /** null = 加载中（行区空态） */
  const [tasks, setTasks] = useState<BackgroundTaskInfo[] | null>(null);
  const [inputs, setInputs] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, string | null>>({});

  useEffect(() => {
    let mounted = true;
    listBackgroundTasks()
      .then((list) => {
        if (!mounted) return;
        setTasks(list);
        setInputs(Object.fromEntries(list.map((t) => [t.taskId, String(t.intervalSec)])));
      })
      .catch((e) => {
        console.error("加载后台任务清单失败", e);
        if (mounted) setTasks([]);
      });
    return () => {
      mounted = false;
    };
  }, []);

  /** 提交成功公共收尾：返回清单更新行 + 前端调度器即时生效 + planBalance 反馈闭环 */
  const afterCommitted = useCallback((list: BackgroundTaskInfo[]) => {
    setTasks(list);
    for (const t of list) {
      if (t.taskId === SESSION_REFRESH_TASK_ID) {
        backgroundTaskScheduler.applyConfig(t.taskId, {
          enabled: t.enabled,
          intervalSec: t.intervalSec,
        });
      }
    }
    if (list.some((t) => t.taskId === PLAN_BALANCE_TASK_ID)) {
      refreshPlanBalance().catch((e) => console.error("刷新套餐余量失败", e));
    }
  }, []);

  /** 勾选切换立即提交（不做乐观更新——本地命令往返快，失败时 UI 保持原值） */
  const handleToggle = useCallback(
    (task: BackgroundTaskInfo, enabled: boolean) => {
      setBackgroundTaskConfig(task.taskId, { enabled })
        .then(afterCommitted)
        .catch((e) => toast.show("warning", `设置失败：${getErrorMessage(e)}`));
    },
    [afterCommitted],
  );

  /** 频率失焦/回车提交：非法 → 行内红字不提交不 toast；合法 → 命令 + 规范化回显 */
  const handleCommitInterval = useCallback(
    (task: BackgroundTaskInfo) => {
      const trimmed = (inputs[task.taskId] ?? "").trim();
      const v = Number(trimmed);
      if (
        trimmed === "" ||
        !Number.isFinite(v) ||
        !Number.isInteger(v) ||
        v < task.intervalMin ||
        v > task.intervalMax
      ) {
        setErrors((prev) => ({ ...prev, [task.taskId]: rangeHint(task) }));
        return;
      }
      setErrors((prev) => ({ ...prev, [task.taskId]: null }));
      setBackgroundTaskConfig(task.taskId, { intervalSec: v })
        .then((list) => {
          afterCommitted(list);
          setInputs((prev) => ({ ...prev, [task.taskId]: String(v) }));
        })
        .catch((e) => toast.show("warning", `设置失败：${getErrorMessage(e)}`));
    },
    [inputs, afterCommitted],
  );

  return (
    <div style={{ width: "100%", height: "100%", background: PANEL_BG }} data-e2e="settings-background-tasks-page">
      <div style={{ padding: "16px 20px" }}>
        {(tasks ?? []).map((task) => (
          <TaskRow
            key={task.taskId}
            task={task}
            input={inputs[task.taskId] ?? ""}
            error={errors[task.taskId] ?? null}
            onToggle={(enabled) => handleToggle(task, enabled)}
            onInput={(v) => setInputs((prev) => ({ ...prev, [task.taskId]: v }))}
            onCommitInterval={() => handleCommitInterval(task)}
          />
        ))}
      </div>
    </div>
  );
};

export default BackgroundTasksPage;
