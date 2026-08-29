// PlanBalancePage — 设置中心「套餐余量」配置页（F11，SC-FE-04）
//
// 读取：挂载时经通用 loadSettings 读 planBalance.intervalSec 段（不新增 get 命令、
//       不新建 store，组件内状态）——缺失/越界显示默认 60（与后端 resolve_poll_interval
//       钳制语义对齐，两层不矛盾：GUI 拒绝越界写、手改越界读时回退）。
// 提交：失焦 / 回车提交（非 debounce——避免删空重输的中间态越界）。非法（非数/越界）
//       → 行内红字提示，不提交不 toast；合法 → 专用命令 plan_balance_set_interval
//       （后端校验 10–3600 + settings.rs 写通道落盘 + 内存原子量）→ 成功后调一次
//       refreshPlanBalance() 拉取最新余量（生效反馈闭环）；Err → toast + 保留用户输入。
// 本页立即提交型（无 dirty 暂存），不调用 onDirtyChange。

import React, { useCallback, useEffect, useState } from "react";
import { loadSettings } from "../../../ipc/settings";
import { setPlanBalanceInterval, refreshPlanBalance } from "../../../ipc/planBalance";
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

/** 轮询间隔边界（与后端 plan_balance 常量语义对齐） */
const MIN_INTERVAL_SEC = 10;
const MAX_INTERVAL_SEC = 3600;
const DEFAULT_INTERVAL_SEC = 60;
/** 行内非法提示文案（范围提示 + 默认值，规格 §4.4） */
const INVALID_HINT = "10–3600 秒，默认 60";

const PlanBalancePage: React.FC<SettingsPageProps> = () => {
  const [inputValue, setInputValue] = useState(String(DEFAULT_INTERVAL_SEC));
  const [error, setError] = useState<string | null>(null);

  // 挂载读取：data?.planBalance?.intervalSec 有限数且 10–3600 → 显示之，否则显示 60
  useEffect(() => {
    let mounted = true;
    loadSettings()
      .then((s) => {
        if (!mounted) return;
        const raw = (s.data as { planBalance?: { intervalSec?: unknown } } | null)
          ?.planBalance?.intervalSec;
        const num = typeof raw === "number" ? raw : NaN;
        setInputValue(
          Number.isFinite(num) &&
            num >= MIN_INTERVAL_SEC &&
            num <= MAX_INTERVAL_SEC
            ? String(num)
            : String(DEFAULT_INTERVAL_SEC),
        );
      })
      .catch((e) => {
        // 读设置失败：按默认值显示，不阻塞页面
        if (!mounted) return;
        setInputValue(String(DEFAULT_INTERVAL_SEC));
        console.error("读取设置失败", e);
      });
    return () => {
      mounted = false;
    };
  }, []);

  /** 失焦 / 回车提交：trim → Number 解析 + 整数 + 10–3600 → 非法行内红字不提交不 toast */
  const handleCommit = useCallback(() => {
    const trimmed = inputValue.trim();
    const v = Number(trimmed);
    if (
      trimmed === "" ||
      !Number.isFinite(v) ||
      !Number.isInteger(v) ||
      v < MIN_INTERVAL_SEC ||
      v > MAX_INTERVAL_SEC
    ) {
      setError(INVALID_HINT);
      return;
    }
    setError(null);
    setPlanBalanceInterval(v)
      .then(() => {
        // 生效反馈闭环（规格 §4.4）：成功后拉取最新余量；失败仅防御（console.error）
        refreshPlanBalance().catch((e) =>
          console.error("刷新套餐余量失败", e),
        );
        // 成功：规范化显示（去 trim 残留空白）
        setInputValue(String(v));
      })
      .catch((e) => {
        // 后端拒绝（越界/落盘失败）：toast + 保留用户输入
        toast.show("warning", `设置失败：${getErrorMessage(e)}`);
      });
  }, [inputValue]);

  return (
    <div style={{ width: "100%", height: "100%", background: PANEL_BG }} data-e2e="settings-plan-balance-page">
      <div style={{ padding: "16px 20px" }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            fontSize: 13,
            color: SIDEBAR_FG,
          }}
        >
          <label htmlFor="settings-plan-balance-input">套餐余量查询频率（秒）</label>
          <input
            id="settings-plan-balance-input"
            type="text"
            inputMode="numeric"
            value={inputValue}
            data-e2e="settings-plan-balance-input"
            onChange={(e) => setInputValue(e.target.value)}
            onFocus={(e) => {
              e.currentTarget.style.borderColor = FOCUS_BORDER;
            }}
            onBlur={(e) => {
              e.currentTarget.style.borderColor = INPUT_BORDER;
              handleCommit();
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleCommit();
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
          <span style={{ fontSize: 12, color: DIM_FG }}>{INVALID_HINT}</span>
        </div>
        {error && (
          <div
            data-e2e="settings-plan-balance-error"
            style={{ marginTop: 8, fontSize: 12, color: ERROR_FG }}
          >
            {error}
          </div>
        )}
      </div>
    </div>
  );
};

export default PlanBalancePage;
