/**
 * 侧栏视图域 E2E spec（E2E-09 拆分 + E2E-11 改名）：
 * 点击开关（R1 替换 / R2 关闭）、跨区移动状态机（R6/R7）、双开像素守卫（E2E-3）、
 * 刷新重启恢复（E2E-4——末位，reload 有 helper 真空窗）。
 */

import { expect, browser } from "@wdio/globals";
import { waitForWorkspaceReady, createProject } from "./specUtils";

describe("侧栏视图", () => {
  // E2E-1: 点击活动栏按钮开关侧栏视图（R1 替换、R2 关闭）
  // 验证：createProject 后默认 nav 打开 → 点击 nav 关闭 → 再点恢复 → 点 explorer 替换
  it("点击活动栏按钮开关/替换侧栏视图", async () => {
    // 0. 等待 Workspace 就绪
    await waitForWorkspaceReady();

    // 1. 创建测试项目
    await createProject("C:\\e2e-sidebar-toggle");

    // 2. 等待活动栏按钮渲染（数据已驱，React 需完成渲染）
    await browser.waitUntil(
      async () => await browser.execute(() => {
        return !!document.querySelector('[data-e2e="activity-btn-nav"]');
      }),
      { timeout: 10000, timeoutMsg: "活动栏按钮未渲染" },
    );

    // 3. 将侧栏重置为已知状态（FIX-TE-04：完整 zones+open 重置，覆盖持久化残留 / 前序副作用）
    await browser.execute(() => {
      const move = (window as any).__slterm_e2e_moveSideViewButton;
      const toggle = (window as any).__slterm_e2e_toggleSideView;
      const getState = (window as any).__slterm_e2e_getSideBarState;
      if (typeof move !== "function" || typeof toggle !== "function") return;

      // 所有按钮归位 top 区对应序位：nav(0) / explorer(1) / commit(2)
      move("nav", "top", 0);
      move("explorer", "top", 1);
      move("commit", "top", 2);

      // open 重置为 nav 打开、bottom 关闭。
      // 注意：toggle 是同步 store 操作，但 getState 快照须在每次 toggle 后重读——
      // 旧快照判断（初始 open.top='commit' 等非 nav 值）会导致 toggle 后
      // open.top=null 时「补开 nav」分支误判跳过（实测 E2E settings 残留
      // open.top='commit' 时连败 4 次）。
      const s = getState?.();
      if (s?.open.bottom) toggle(s.open.bottom);
      if (s?.open.top && s.open.top !== "nav") toggle(s.open.top);
      const s2 = getState?.();
      if (!s2?.open.top) toggle("nav");
    });

    // 4. 验证初始状态：项目列表打开（open.top === "nav"）
    const initialState = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });
    expect(initialState).not.toBeNull();
    expect(initialState!.open.top).toBe("nav");
    expect(initialState!.open.bottom).toBeNull();

    // 5. 点击项目列表按钮 → 关闭侧栏区（R2: toggle 关闭）
    await browser.execute(() => {
      const btn = document.querySelector('[data-e2e="activity-btn-nav"]') as HTMLElement;
      btn?.click();
    });

    // 断言侧栏区隐藏（open 双空）
    await browser.waitUntil(
      async () => {
        const s = await browser.execute(() => {
          const fn = (window as any).__slterm_e2e_getSideBarState;
          return typeof fn === "function" ? fn() : null;
        });
        return s && s.open.top === null && s.open.bottom === null ? s : false;
      },
      { timeout: 5000, timeoutMsg: "侧栏区未在点击后关闭（open 双空）" },
    );

    // 6. 再次点击 → 恢复项目列表
    await browser.execute(() => {
      const btn = document.querySelector('[data-e2e="activity-btn-nav"]') as HTMLElement;
      btn?.click();
    });

    await browser.waitUntil(
      async () => {
        const s = await browser.execute(() => {
          const fn = (window as any).__slterm_e2e_getSideBarState;
          return typeof fn === "function" ? fn() : null;
        });
        return s && s.open.top === "nav" ? s : false;
      },
      { timeout: 5000, timeoutMsg: "侧栏区未恢复（open.top !== \"nav\"）" },
    );

    // 7. 点击文件浏览器 → R1 替换：explorer 替换 nav（单槽位覆盖）
    await browser.execute(() => {
      const btn = document.querySelector('[data-e2e="activity-btn-explorer"]') as HTMLElement;
      btn?.click();
    });

    const explorerState = await browser.waitUntil(
      async () => {
        const s = await browser.execute(() => {
          const fn = (window as any).__slterm_e2e_getSideBarState;
          return typeof fn === "function" ? fn() : null;
        });
        return s && s.open.top === "explorer" ? s : false;
      },
      { timeout: 5000, timeoutMsg: "explorer 未替换 nav（R1 替换失败）" },
    );
    expect(explorerState.open.top).toBe("explorer");
    expect(explorerState.open.bottom).toBeNull(); // 单槽位：仅一区有视图
  });

  // E2E-2: 侧栏视图跨区移动状态机（R6/R7）——E2E-11 改名：实际走 store helper
  // （__slterm_e2e_moveSideViewButton），非真实 HTML5 拖拽。
  //
  // DnD 合成依赖 DataTransfer 构造器（Chromium/WebView2 ≥ 85）。
  // 由于活动栏区容器缺少 data-e2e，合成事件通过 DOM 导航定位下区容器；
  // 若 DataTransfer 不可用或 DOM 结构不匹配，降级 __slterm_e2e_moveSideViewButton 驱动。
  //
  // 状态机验证覆盖：
  //   R6 — 已打开视图跨区移动时 open 跟随到目标区
  //   R7 — 未打开视图移动时 open 不变
  //
  // 人工验收项：拖拽手感、插入指示线位置、跨区落点视觉反馈（合成事件无法模拟鼠标坐标）
  it("侧栏视图跨区移动状态机（R6/R7）", async () => {
    // 0. 等待 Workspace 就绪
    await waitForWorkspaceReady();

    // 1. 创建测试项目
    await createProject("C:\\e2e-sidebar-dnd");

    // 2. 等待活动栏渲染
    await browser.waitUntil(
      async () => await browser.execute(() => {
        return !!document.querySelector('[data-e2e="activity-btn-explorer"]');
      }),
      { timeout: 10000, timeoutMsg: "活动栏按钮未渲染" },
    );

    // 3. 将侧栏重置为已知状态（FIX-TE-04：完整 zones+open 重置，避免持久化残留影响拖拽前的预期）
    await browser.execute(() => {
      const move = (window as any).__slterm_e2e_moveSideViewButton;
      const toggle = (window as any).__slterm_e2e_toggleSideView;
      const getState = (window as any).__slterm_e2e_getSideBarState;
      if (typeof move !== "function" || typeof toggle !== "function") return;

      // 所有按钮归位 top 区对应序位：nav(0) / explorer(1) / commit(2)
      move("nav", "top", 0);
      move("explorer", "top", 1);
      move("commit", "top", 2);

      // open 重置为 explorer 打开、bottom 关闭
      const s = getState?.();
      if (s?.open.bottom) toggle(s.open.bottom);
      if (s?.open.top && s.open.top !== "explorer") toggle(s.open.top);
      if (!s?.open.top) toggle("explorer");
    });

    // 4. 验证初始 zones：explorer 在上区
    let state = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });
    expect(state!.zones.top).toContain("explorer");
    expect(state!.zones.bottom).not.toContain("explorer");

    // 5. 确保 explorer 视图已打开（R6 跟随验证需 explorer 是打开的）
    state = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });

    if (state!.open.top !== "explorer") {
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("explorer");
      });
    }

    state = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });
    expect(state!.open.top).toBe("explorer");

    // 6. 用 helper 移动 explorer 到下区（验证状态机 R6）
    //    合成 DnD 事件在 E2E 环境中缺少 clientY，zone 检测失效；
    //    helper 直调 store.moveButton 避开 DOM 层竞态，R6/R7 状态断言不受影响。
    await browser.execute(() => {
      (window as any).__slterm_e2e_moveSideViewButton?.("explorer", "bottom", 0);
    });

    // 7. 断言 R6: zones 变化 + open 跟随到目标区
    state = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });
    expect(state!.zones.top).not.toContain("explorer");
    expect(state!.zones.bottom).toContain("explorer");
    // R6: explorer 在上区打开时移到下区 → open.bottom 跟随设为 "explorer"
    expect(state!.open.bottom).toBe("explorer");
    // 原区 top 在 explorer 移走后置 null（被替换）
    expect(state!.open.top).toBeNull();

    // 8. 验证 R7：未打开视图移动时 open 不跟随
    //    8a. 先把 explorer 移回上区
    await browser.execute(() => {
      (window as any).__slterm_e2e_moveSideViewButton?.("explorer", "top", 0);
    });

    state = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });
    // explorer 回到上区，open.bottom 被清除，open.top 跟随设为 "explorer"（R6）
    expect(state!.zones.top).toContain("explorer");
    expect(state!.zones.bottom).not.toContain("explorer");

    //    8b. 关闭 explorer（toggleView 置 null）
    await browser.execute(() => {
      (window as any).__slterm_e2e_toggleSideView?.("explorer");
    });

    state = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });
    expect(state!.open.top).toBeNull();
    expect(state!.open.bottom).toBeNull();

    //    8c. 此时移动 explorer 到下区 → open 应不变（R7: 未打开不跟随）
    await browser.execute(() => {
      (window as any).__slterm_e2e_moveSideViewButton?.("explorer", "bottom", 0);
    });

    state = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });
    expect(state!.zones.top).not.toContain("explorer");
    expect(state!.zones.bottom).toContain("explorer");
    // R7: explorer 未打开，移动后 open 不变
    expect(state!.open.top).toBeNull();
    expect(state!.open.bottom).toBeNull();
  });

  // E2E-3: 双开下区像素高度防复发（2026-09 preferredSize 单位 bug——像素误植致下区
  // 只剩 ≈30px 标题高度 + onChange 写回棘轮；L2 锁百分比字符串契约，本条锁真实像素结果）
  it("首开下区双视图：下区槽位获得均分级像素高度（>100px，非标题高度）", async () => {
    // 1. 等活动栏按钮渲染
    await browser.waitUntil(
      async () => {
        return await browser.execute(() => {
          return !!document.querySelector('[data-e2e="activity-btn-nav"]');
        });
      },
      { timeout: 10000, timeoutMsg: "活动栏按钮未渲染" },
    );

    // 2. 重置侧栏到已知态（照 E2E-1：zones 归位 + open 单开 nav）
    await browser.execute(() => {
      const move = (window as any).__slterm_e2e_moveSideViewButton;
      const toggle = (window as any).__slterm_e2e_toggleSideView;
      const getState = (window as any).__slterm_e2e_getSideBarState;
      if (typeof move !== "function" || typeof toggle !== "function") return;
      move("nav", "top", 0);
      move("explorer", "top", 1);
      move("commit", "top", 2);
      const s = getState?.();
      if (s?.open.bottom) toggle(s.open.bottom);
      if (s?.open.top && s.open.top !== "nav") toggle(s.open.top);
      const s2 = getState?.();
      if (!s2?.open.top) toggle("nav");
    });

    // 3. explorer 移到下区（R7 不动 open）→ toggle 打开（R1：open.bottom = explorer，双开）
    await browser.execute(() => {
      (window as any).__slterm_e2e_moveSideViewButton?.("explorer", "bottom", 0);
      (window as any).__slterm_e2e_toggleSideView?.("explorer");
    });

    // 4. 等双开落定后断言下区槽位像素高度——旧 bug 下 ≈30px（库 minSize clamp）
    await browser.waitUntil(
      async () => {
        return await browser.execute(() => {
          const s = (window as any).__slterm_e2e_getSideBarState?.();
          return s && s.open.top === "nav" && s.open.bottom === "explorer";
        });
      },
      { timeout: 5000, timeoutMsg: "双开未落定（open.top/bottom 不符合预期）" },
    );
    const bottomHeight = await browser.execute(() => {
      const el = document.querySelector(
        '[data-e2e="sidebar-slot-bottom-explorer"]',
      ) as HTMLElement | null;
      return el?.offsetHeight ?? 0;
    });
    expect(bottomHeight).toBeGreaterThan(100);

    // 5. 收尾：关下区 + explorer 归位 top（防持久化残留影响后续 spec——settings 落盘有 2s debounce）
    await browser.execute(() => {
      const toggle = (window as any).__slterm_e2e_toggleSideView;
      const move = (window as any).__slterm_e2e_moveSideViewButton;
      toggle?.("explorer");
      move?.("explorer", "top", 1);
    });
  });

  // E2E-4: 刷新重启防复发（2026-09 二轮纠偏回归——Bug 1：双开持久化态重启即
  // 「页面渲染出错」（mount 期 viewItems 未 populate 时调 resize 崩溃）；
  // Bug 2：关闭帧瞬时 fire [H,0] 经旧闭包污染 splitRatio=0.9，重开下区仅 ≈30px）。
  //
  // 机制说明：embedded driver 的 WebDriver server 运行在 app 进程内，杀 app 后
  // session 不可重建（tauri-service 无 spec 内 respawn 钩子）——故重启等价物取
  // location.reload()：渲染链整树重挂 + 持久化状态重读，与进程重启的崩溃/恢复
  // 路径完全一致（两 bug 均纯渲染侧）；Rust 侧状态不在本回归面内。
  //
  // 本用例须位于本 spec 末位：reload 重置 window 全部 helper（bootstrap 重装前
  // 有真空窗），后续用例须重新等 workspaceReady。
  it("双开状态刷新重启：无渲染错误页、下区恢复均分级像素高度（启动崩溃 + 比例污染防复发）", async () => {
    // 1. 等活动栏按钮渲染
    await waitForWorkspaceReady();
    await browser.waitUntil(
      async () => await browser.execute(() => {
        return !!document.querySelector('[data-e2e="activity-btn-nav"]');
      }),
      { timeout: 10000, timeoutMsg: "活动栏按钮未渲染" },
    );

    // 2. 重置侧栏到已知态（照 E2E-1：zones 归位 + open 单开 nav）
    await browser.execute(() => {
      const move = (window as any).__slterm_e2e_moveSideViewButton;
      const toggle = (window as any).__slterm_e2e_toggleSideView;
      const getState = (window as any).__slterm_e2e_getSideBarState;
      if (typeof move !== "function" || typeof toggle !== "function") return;
      move("nav", "top", 0);
      move("explorer", "top", 1);
      move("commit", "top", 2);
      const s = getState?.();
      if (s?.open.bottom) toggle(s.open.bottom);
      if (s?.open.top && s.open.top !== "nav") toggle(s.open.top);
      const s2 = getState?.();
      if (!s2?.open.top) toggle("nav");
    });

    // 3. explorer 移到下区并打开（双开）；等双开落定
    await browser.execute(() => {
      (window as any).__slterm_e2e_moveSideViewButton?.("explorer", "bottom", 0);
      (window as any).__slterm_e2e_toggleSideView?.("explorer");
    });
    await browser.waitUntil(
      async () => {
        return await browser.execute(() => {
          const s = (window as any).__slterm_e2e_getSideBarState?.();
          return s && s.open.top === "nav" && s.open.bottom === "explorer";
        });
      },
      { timeout: 5000, timeoutMsg: "双开未落定（open.top/bottom 不符合预期）" },
    );

    // 4. 等持久化落盘（sideBar store 2s debounce 保存 + 裕度）
    await new Promise((r) => setTimeout(r, 3000));

    // 5. 刷新重启——setTimeout 异步触发：同步 location.reload() 会让 executeScript
    //    的求值上下文随导航销毁、驱动永不回包（plugin refresh 端点同机理死锁），
    //    先调度后返回则 HTTP 响应在导航前完成。随后先等 helper 真空窗确认重启发生
    await browser.execute(() => {
      setTimeout(() => location.reload(), 50);
    });
    await browser.waitUntil(
      async () => await browser.execute(() => {
        return (
          typeof (window as any).__slterm_e2e_getSideBarState === "undefined"
        );
      }),
      { timeout: 8000, timeoutMsg: "刷新未发生（helper 真空窗未出现）" },
    );

    // 6. 等渲染链整树重挂 + 双开从持久化恢复（bootstrap 重装 helper 后
    //    workspaceReady 重新置位，再读 store）
    await waitForWorkspaceReady(20000);
    await browser.waitUntil(
      async () => {
        return await browser.execute(() => {
          const s = (window as any).__slterm_e2e_getSideBarState?.();
          return s && s.loaded && s.open.top === "nav" && s.open.bottom === "explorer";
        });
      },
      { timeout: 10000, timeoutMsg: "重启后双开未从持久化恢复" },
    );

    // 7. Bug 1 断言：无「页面渲染出错」错误页
    const hasErrorPage = await browser.execute(() => {
      return document.body.innerText.includes("页面渲染出错");
    });
    expect(hasErrorPage).toBe(false);

    // 8. Bug 2 断言：下区槽位恢复均分级像素高度（污染 0.9 时仅 ≈30px）
    const bottomHeight = await browser.execute(() => {
      const el = document.querySelector(
        '[data-e2e="sidebar-slot-bottom-explorer"]',
      ) as HTMLElement | null;
      return el?.offsetHeight ?? 0;
    });
    expect(bottomHeight).toBeGreaterThan(100);

    // 9. 收尾：关下区 + explorer 归位 top（防持久化残留影响后续 spec）
    await browser.execute(() => {
      const toggle = (window as any).__slterm_e2e_toggleSideView;
      const move = (window as any).__slterm_e2e_moveSideViewButton;
      toggle?.("explorer");
      move?.("explorer", "top", 1);
    });
  });
});
