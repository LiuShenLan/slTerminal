// doc-viewer-floating-area.test.tsx — 右上悬浮区（FloatingArea + useZoomHud）行为测试
//
// 2026-09-06 收敛产物：缩放 HUD 状态机与切换条的空间协调单点（原 PreviewFrame
// overlay 槽 / 面板根双轨 → md HUD 与切换条重叠缺陷的防回归面）。锁定语义：
//   - report(zoom) 变化即显示（含回落 100%——Chrome 缩放气泡语义，防误判复位）
//   - 等值回声（复位回声 / 超时后同值回声）不复活已隐藏气泡
//   - hide() = 静默归 1（重置 / iframe 重建通知），非「显示 100%」
//   - 3s 无操作自动消失、期间缩放续期（fake timers）
//   - 悬浮区列排次序：切换条上 / HUD 下（重叠缺陷几何面）；data-e2e 前缀探针

import { describe, it, expect, vi, afterEach } from "vitest";
import React from "react";
import { act, cleanup, fireEvent, render, renderHook } from "@testing-library/react";
import { FloatingArea } from "../panels/docViewer/FloatingArea";
import { useZoomHud } from "../panels/docViewer/useZoomHud";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("FloatingArea 悬浮区列排", () => {
  it("hud=null 仅渲染切换条（edit 形态无缩放源）", () => {
    const { container } = render(
      <FloatingArea dataE2ePrefix="test" switcher={<div data-testid="fake-switcher" />} />,
    );
    expect(container.querySelector('[data-testid="fake-switcher"]')).not.toBeNull();
    expect(container.querySelector('[data-e2e="test-zoom-hud"]')).toBeNull();
  });

  it("hud visible 时气泡渲染于切换条下方（列排次序——原重叠缺陷根因面）", () => {
    const { container } = render(
      <FloatingArea
        dataE2ePrefix="test"
        switcher={<div data-testid="fake-switcher" />}
        hud={{ zoom: 1.5, visible: true, onReset: vi.fn() }}
      />,
    );
    const hudEl = container.querySelector<HTMLElement>('[data-e2e="test-zoom-hud"]');
    expect(hudEl).not.toBeNull();
    expect(hudEl!.textContent).toContain("150%");
    // HUD 紧邻切换条之后（气泡不与切换条重叠的 DOM 形态保证）
    const prev = hudEl!.previousElementSibling;
    expect(prev?.querySelector('[data-testid="fake-switcher"]')).not.toBeNull();
  });

  it("visible=false 不渲染气泡；重置按钮探针命名 + 点击调 onReset", () => {
    const onReset = vi.fn();
    const { container } = render(
      <FloatingArea dataE2ePrefix="test" hud={{ zoom: 1, visible: false, onReset }} />,
    );
    expect(container.querySelector('[data-e2e="test-zoom-hud"]')).toBeNull();

    // 重新渲染为 visible（同容器替换——瞬态气泡随状态挂载）
    const { container: r2 } = render(
      <FloatingArea
        dataE2ePrefix="test"
        switcher={undefined}
        hud={{ zoom: 1.21, visible: true, onReset }}
      />,
    );
    const resetBtn = r2.querySelector<HTMLButtonElement>('[data-e2e="test-zoom-reset"]');
    expect(resetBtn).not.toBeNull();
    fireEvent.click(resetBtn!);
    expect(onReset).toHaveBeenCalledTimes(1);
  });
});

describe("useZoomHud 状态机", () => {
  it("report 变化即显示：150% → 回落 100% 仍显示气泡（变化非复位——防回归）", () => {
    const { result } = renderHook(() => useZoomHud());
    expect(result.current.hud.visible).toBe(false);

    act(() => result.current.report(1.5));
    expect(result.current.hud).toEqual({ zoom: 1.5, visible: true });

    // 回落 1.0 属缩放变化（非 iframe 重建/重置）——显示 100% 气泡，超时才消失
    act(() => result.current.report(1.0));
    expect(result.current.hud).toEqual({ zoom: 1, visible: true });
  });

  it("等值回声不复活已隐藏气泡（超时后同值回声 / 复位回声均忽略）", () => {
    const { result } = renderHook(() => useZoomHud());
    vi.useFakeTimers();
    act(() => result.current.report(1.5));
    expect(result.current.hud.visible).toBe(true);
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(result.current.hud.visible).toBe(false);
    // 超时自动隐藏后同值回声（last 未归 1，等值判断命中）→ 不复活
    act(() => result.current.report(1.5));
    expect(result.current.hud.visible).toBe(false);

    // hide（重置链）后复位回声 1.0：last 已归 1 → 等值忽略
    act(() => result.current.report(2.0));
    act(() => result.current.hide());
    expect(result.current.hud).toEqual({ zoom: 1, visible: false });
    act(() => result.current.report(1.0));
    expect(result.current.hud.visible).toBe(false);
  });

  it("hide 后新变化值重新显示（新缩放会话）", () => {
    const { result } = renderHook(() => useZoomHud());
    act(() => result.current.report(1.21));
    act(() => result.current.hide());
    act(() => result.current.report(1.44));
    expect(result.current.hud).toEqual({ zoom: 1.44, visible: true });
  });

  it("3s 无操作自动消失；期间缩放续期（每次变化重置计时）", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useZoomHud());
    act(() => result.current.report(1.1));
    act(() => {
      vi.advanceTimersByTime(2000);
    });
    expect(result.current.hud.visible).toBe(true);
    // 2s 时续期（变化）→ 距上次变化不足 3s 仍显示
    act(() => result.current.report(1.21));
    act(() => {
      vi.advanceTimersByTime(2000);
    });
    expect(result.current.hud.visible).toBe(true);
    // 续期后满 3s（距第二次变化 3s）→ 消失
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(result.current.hud.visible).toBe(false);
  });

  it("组合链路：report 驱动 → 气泡出现 → 重置钮点击 → 消失（面板同构面）", () => {
    function Harness(): React.JSX.Element {
      const zoomHud = useZoomHud();
      return (
        <>
          <button data-testid="zoom-drive" onClick={() => zoomHud.report(1.5)} />
          <FloatingArea
            dataE2ePrefix="test"
            switcher={<div data-testid="fake-switcher" />}
            hud={{
              zoom: zoomHud.hud.zoom,
              visible: zoomHud.hud.visible,
              onReset: zoomHud.hide,
            }}
          />
        </>
      );
    }
    const { container } = render(<Harness />);
    expect(container.querySelector('[data-e2e="test-zoom-hud"]')).toBeNull();
    // 缩放变化上报（面板经 PreviewFrame onZoomChange → hook.report）
    fireEvent.click(container.querySelector('[data-testid="zoom-drive"]')!);
    const hudEl = container.querySelector<HTMLElement>('[data-e2e="test-zoom-hud"]');
    expect(hudEl).not.toBeNull();
    expect(hudEl!.textContent).toContain("150%");
    // 重置点击（悬浮区 onReset = hide，下行复位在面板侧经 PreviewFrame ref）
    fireEvent.click(container.querySelector('[data-e2e="test-zoom-reset"]')!);
    expect(container.querySelector('[data-e2e="test-zoom-hud"]')).toBeNull();
  });
});
