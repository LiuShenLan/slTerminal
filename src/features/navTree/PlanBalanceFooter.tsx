// F10 套餐余量 footer——树滚动区与「添加项目」钮之间的固定区（U1）
// 全部来源无展示 → 整块不渲染（含发丝线，规格 §8.3）；行点击 = 立即刷新（节流 5s）
import React, { useState } from "react";
import type { PlanBalanceInfo } from "../../types/planBalance";
import { DIM_FG, SEPARATOR_BG } from "../../theme";
import { nameStyle, ROW_HEIGHT, rowBaseStyle } from "./navStyles";
import { planLogoSrc, rowText, rowTooltip } from "./planBalanceModel";
import { usePlanBalance } from "./usePlanBalance";

const PlanBalanceRow: React.FC<{ info: PlanBalanceInfo; onRefresh: () => void }> = ({
  info, onRefresh,
}) => {
  const [hovered, setHovered] = useState(false);
  // logo 文件缺失 → onError 隐藏仅显文本（规格 §8.1 不裂图）
  const [logoFailed, setLogoFailed] = useState(false);
  return (
    <div
      data-e2e="plan-balance-row"
      onClick={onRefresh}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      title={rowTooltip(info, Date.now())}
      style={{
        ...rowBaseStyle(false, hovered, ROW_HEIGHT),
        color: DIM_FG, // fg-3（§8.1）
        fontSize: 12,
      }}
    >
      {!logoFailed && (
        <img
          src={planLogoSrc(info.planId)}
          width={14} height={14}
          style={{ flexShrink: 0, display: "block" }}
          alt=""
          onError={() => setLogoFailed(true)}
        />
      )}
      <span style={nameStyle}>{rowText(info)}</span>
    </div>
  );
};

export const PlanBalanceFooter: React.FC = () => {
  const { items, refresh, enabled } = usePlanBalance();
  // 禁用即整块不渲染（F12 enabled 语义：禁用即不关注，快照保留——重启用即重显最后快照）
  if (enabled !== true || items.length === 0) return null; // §8.3：整块（含发丝线）不渲染
  return (
    <div
      data-e2e="plan-balance-footer"
      style={{
        borderTop: `1px solid ${SEPARATOR_BG}`, // 发丝线（§8.1）
        flexShrink: 0,
        padding: "0 8px",
      }}
    >
      {items.map((info) => (
        <PlanBalanceRow key={info.sourceId} info={info} onRefresh={refresh} />
      ))}
    </div>
  );
};
