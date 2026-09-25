// SideBarArea — 侧栏区组件
//
// 活动栏与主区之间的共享展示区域，垂直划分为上区与下区两个半区。
// 每半区一槽位，视图经条件渲染切换（FE-21）。视图跨挂载状态（展开集等）上移
// sideViewRegistry 状态槽（CP-016）——以视图 id 为键，组件经 viewState/onViewStateChange
// 受控消费；换区/槽位切换重建后由回填恢复，不再依赖组件内部 state。

import React, { useEffect, useRef } from "react";
import { Allotment } from "allotment";
import type { AllotmentHandle } from "allotment";
import "allotment/dist/style.css";
import { useSideBar } from "../../stores/sideBar";
import { sideViewRegistry } from "./sideViewRegistry";
import {
  SPLIT_DEFAULT,
  SPLIT_MIN,
  SPLIT_MAX,
} from "./sideBarState";
import { PANEL_BG } from "../../theme/colors";

/** SideBarArea 外部注入 props——与 SidebarTree props 精确匹配 */
export interface SideBarAreaProps {
  /** 切换操作页面（async——切换完成后再开面板） */
  switchToPage: (projectId: string, pageId: string) => Promise<void>;
  /** 删除操作页面 */
  onDeletePage: (projectId: string, pageId: string) => void;
}

/**
 * 侧栏区组件
 *
 * 结构：<Allotment vertical proportionalLayout> 两 pane
 * - 上 pane visible={!!open.top}；下 pane visible={!!open.bottom}
 * - pane 不传 preferredSize（该 prop 无法承载比例语义——number=绝对像素、
 *   百分比字符串在挂载路径被 Math.round 成 NaN）；splitRatio → 像素由
 *   「比例像素纠偏」effect 在双开时经 ref.resize() 主动应用
 * - 每 pane 内：zones[zone].map(id → registry.get(id)) 过滤 undefined →
 *   按单槽位过滤 open 视图后条件渲染（FE-21：隐藏视图卸载，不保挂载），height: 100%
 * - onChange 三道闸（纠偏窗口屏蔽 / 零尺寸跳过 / getState live 双开判定）后
 *   换算 ratio 写回 store——onDidChange 闭包慢一个 commit，禁读渲染闭包状态
 *
 * 硬约束 #6：全部颜色引用 theme/colors.ts token，禁止硬编码色值
 */
export const SideBarArea: React.FC<SideBarAreaProps> = ({
  switchToPage,
  onDeletePage,
}) => {
  const zones = useSideBar((s) => s.zones);
  const open = useSideBar((s) => s.open);
  const splitRatio = useSideBar((s) => s.splitRatio);
  const setSplitRatio = useSideBar((s) => s.setSplitRatio);

  // 各半区已注册视图定义（过滤持久化中可能已取消注册的 id）
  const topDefs = zones.top
    .map((id) => sideViewRegistry.get(id))
    .filter((d): d is NonNullable<typeof d> => d !== undefined);

  const bottomDefs = zones.bottom
    .map((id) => sideViewRegistry.get(id))
    .filter((d): d is NonNullable<typeof d> => d !== undefined);

  const topOpen = open.top !== null;
  const bottomOpen = open.bottom !== null;
  const bothOpen = topOpen && bottomOpen;

  // 首次双开 splitRatio 回退（FE-19）——比例视觉恢复由下方「比例像素纠偏」effect
  // 承载；本 effect 仅作 store 值归一化兜底：仅当 splitRatio 为默认值（无持久化值，
  // 首次进入双视图）或越界（出 [SPLIT_MIN, SPLIT_MAX]）才回退默认 0.5；
  // 用户调节过的合法比例在正常单↔双切换中保留。
  const prevBothOpen = useRef(bothOpen);
  useEffect(() => {
    if (bothOpen && !prevBothOpen.current) {
      const outOfRange =
        splitRatio < SPLIT_MIN || splitRatio > SPLIT_MAX;
      const noPersistedValue = splitRatio === SPLIT_DEFAULT;
      if (outOfRange || noPersistedValue) {
        setSplitRatio(SPLIT_DEFAULT);
      }
    }
    prevBothOpen.current = bothOpen;
  }, [bothOpen, splitRatio, setSplitRatio]);

  // 比例像素纠偏（2026-09 二轮重写）：Allotment 的 preferredSize 无法承载比例语义——
  // number 是绝对像素；百分比字符串在挂载路径被原值喂给 resizeView
  // （库内 Math.round("NN%") → NaN）。故比例恢复 = 双开（挂载即双开 = 重启恢复 /
  // 单→双转换 = 首开或重开下区）时按 splitRatio 快照计算像素调 ref.resize()。
  // 三条库坑决定机制形态（modern.mjs v1.20.5 实证）：
  // ① viewItems 异步 populate——view 加入走「ResizeObserver 首测 → setState →
  //    update effect 才 addView」异步链，mount commit 内 viewItems 必为空；
  //    此时调 resize() 读 undefined.minimumSize 直接崩溃（Bug 1），且 this.size=0
  //    时 resizeViews 的 clamp 会把请求钉成 [30,30]。故必须等「就绪探针」——
  //    首个 sizes.length===2 的 onChange fire 标志 viewItems 已 populate。
  // ② onDidChange 回调慢一个 commit 换闭包——关闭下区 commit 的 layout 期
  //    setViewVisible(false) 同步 fire onChange [H,0]，用的是上一 commit
  //    bothOpen=true 的旧闭包 → ratio=1.0 → clamp 0.9 污染 store（Bug 2）。
  //    故 onChange 不得读渲染闭包状态，一律 getState() live 判定。
  // ③ mount 链瞬时 fire [H-30,30]/[H/2,H/2] 会冲掉持久化比例——故纠偏窗口期
  //    （pendingCorrectionRef）屏蔽一切写回，比例用进入窗口前的同步快照。
  const rootRef = useRef<HTMLDivElement>(null);
  const allotmentRef = useRef<AllotmentHandle>(null);
  const readyRef = useRef(false);
  const pendingCorrectionRef = useRef(false);
  useEffect(() => {
    if (!bothOpen) return;
    // 同步快照 + 窗口标记（抢在后续一切 onChange fire 之前；FE-19 effect 声明在
    // 前，setSplitRatio 同步生效，快照读到的是归一化后的值）
    const ratio = useSideBar.getState().splitRatio;
    pendingCorrectionRef.current = true;
    let raf = 0;
    let tries = 0;
    const tick = (): void => {
      const h = rootRef.current?.clientHeight ?? 0;
      if (readyRef.current && h > 0) {
        allotmentRef.current?.resize([
          Math.round(h * ratio),
          Math.round(h * (1 - ratio)),
        ]);
        // resize 内部同步 fire 的恒等 onChange 已被窗口标记吞掉，此刻放行写回
        pendingCorrectionRef.current = false;
        return;
      }
      // 有界重试 ~60 帧；放弃路径同样放行写回（退化安全，不永久阻塞正常写回）
      if (++tries >= 60) {
        pendingCorrectionRef.current = false;
        return;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [bothOpen]);

  return (
    <div
      ref={rootRef}
      style={{
        height: "100%",
        background: PANEL_BG,
      }}
    >
      <Allotment
        ref={allotmentRef}
        vertical
        proportionalLayout={true}
        onChange={(sizes) => {
          // 就绪探针先于一切闸：首个双 pane fire 标志 viewItems 已 populate，
          // 此后调 resize() 不会再读到空 viewItems（Bug 1 崩溃点）
          if (sizes.length === 2) readyRef.current = true;
          // 闸一：纠偏窗口期屏蔽写回——mount 链瞬时 fire [H-30,30]/[H/2,H/2]
          // 会冲掉持久化比例；纠偏自身 resize 的恒等 fire 也由此吞掉
          if (pendingCorrectionRef.current) return;
          // 闸二：零尺寸 pane 无比例语义——关闭/单开瞬时 fire [H,0] 若写回
          // 得 ratio=1.0 → clamp 0.9 污染 store（Bug 2 放大器）
          if (sizes.some((s) => s <= 0)) return;
          // 闸三：live 判定双开——onDidChange 闭包慢一个 commit（Bug 2 根因），
          // 渲染闭包的 bothOpen/open 都可能是上一 commit 旧值，必须 getState()
          const openNow = useSideBar.getState().open;
          if (openNow.top === null || openNow.bottom === null) return;
          const total = sizes[0] + sizes[1];
          if (total <= 0) return; // 除零守卫
          setSplitRatio(sizes[0] / total);
        }}
      >
        {/* 上区 pane — FE-21：条件渲染替代 display:none 保挂载，仅渲染打开的视图（单槽位）。
            不传 preferredSize：该 prop 无法承载比例语义（number=绝对像素、百分比字符串在
            挂载路径 NaN——见上方「比例像素纠偏」注释），比例由双开时的 ref.resize 纠偏承载 */}
        <Allotment.Pane
          visible={topOpen}
        >
          <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
            {topDefs
              .filter((def) => def.id === open.top)
              .map((def) => (
                <div
                  key={def.id}
                  style={{ height: "100%", display: "flex", flexDirection: "column" }}
                  data-e2e={`sidebar-slot-top-${def.id}`}
                >
                  <def.component
                    switchToPage={switchToPage}
                    onDeletePage={onDeletePage}
                    viewState={sideViewRegistry.getViewState(def.id)}
                    onViewStateChange={(state) =>
                      sideViewRegistry.setViewState(def.id, state)
                    }
                  />
                </div>
              ))}
          </div>
        </Allotment.Pane>

        {/* 下区 pane — FE-21：同上一区，切换即卸载旧视图组件（状态经注册表状态槽回填恢复）。
            preferredSize 不传（同上区注释） */}
        <Allotment.Pane
          visible={bottomOpen}
        >
          <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
            {bottomDefs
              .filter((def) => def.id === open.bottom)
              .map((def) => (
                <div
                  key={def.id}
                  style={{ height: "100%", display: "flex", flexDirection: "column" }}
                  data-e2e={`sidebar-slot-bottom-${def.id}`}
                >
                  <def.component
                    switchToPage={switchToPage}
                    onDeletePage={onDeletePage}
                    viewState={sideViewRegistry.getViewState(def.id)}
                    onViewStateChange={(state) =>
                      sideViewRegistry.setViewState(def.id, state)
                    }
                  />
                </div>
              ))}
          </div>
        </Allotment.Pane>
      </Allotment>
    </div>
  );
};
