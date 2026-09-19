// usePanelActivationFocus — 面板激活联动键盘焦点共享 hook（C4「新建即聚焦」）
//
// 两条焦点驱动路径：
// ① 挂载期意图消费：ready && api.isActive && api.isGroupActive &&
//    consumePanelFocusIntent(panelId) → focus()——dockview addPanel 默认激活
//    新面板，挂载即激活的打开入口（新建终端/编辑器/git/设置/恢复注入）经
//    panelFocusIntent 令牌驱动；ready 翻 true（异步资源就绪，如 xterm 容器
//    挂载、CM 视图创建）时补消费一次。
//    **focus 经 requestAnimationFrame 延迟一帧执行**（2026-09-19 L4 取证）：
//    dockview 对 renderer:"always" 面板用 overlay 渲染容器（dv-render-overlay），
//    attach 时 visibility:hidden，可见性要到下一帧 rAF（attach 微任务 →
//    resize → rAF）才翻开——同帧 passive effect 内 focus() 对
//    visibility:hidden 元素静默无效（不产焦点事件；jsdom 不校验 CSS 可见性，
//    L2 测不出，勿回退为同步 focus）。rAF 注册序保证 dockview 的翻开回调
//    先执行（其注册于 addPanel 同步段微任务，早于本 effect 的宏任务调度）；
//    rAF 间隙激活态可能翻转（极速切页），回调内重校验双条件。
// ② 激活事件联动：onDidActiveChange / onDidActiveGroupChange（可选调用照
//    DefaultTab 先例——测试 fake api 可能缺）双条件满足即 focus()——**不需
//    意图**：覆盖页签点击激活、去重命中 panel.focus()、switchToPageAndFocus
//    全路径。该路径触发时面板早已可见（用户见得到才点得到），保持同步。
//
// 布局批量恢复（fromJSON）豁免：恢复路径从不写意图，路径①不命中；恢复
// 不产激活事件（dockview 按 activeGroup 字段静默置位），路径②不命中——
// 重启恢复后无面板抢焦。
//
// 焦点目标由调用方面板注入（focus 回调）：终端 = xterm 输入区，编辑器 =
// CM 视图，设置 = 壳内容容器——本 hook 只管「何时聚焦」，不管「聚焦到哪」。

import { useEffect } from "react";
import type { DockviewPanelApi } from "dockview-react";
import { consumePanelFocusIntent } from "../workspace/panelFocusIntent";

/**
 * 面板激活联动键盘焦点。
 * @param api 面板 DockviewPanelApi（面板 props 传入）
 * @param panelId 面板 id（页前缀协议 id——意图令牌键）
 * @param focus 焦点落点回调（幂等——多次调用安全）
 * @param ready 焦点目标就绪标记（缺省 true；异步资源（xterm 容器/CM 视图）
 *   未就绪时 false，翻 true 时补消费挂载期意图）
 */
export function usePanelActivationFocus(
  api: DockviewPanelApi,
  panelId: string,
  focus: () => void,
  ready: boolean = true,
): void {
  // 路径①：挂载期意图消费（ready 翻 true 补消费——effect 依赖 ready 重跑，
  // 意图 take 语义保证只消费一次）。
  // deps 不含 focus：回调由调用方 useCallback 稳定化（照 TerminalPanel
  // handleFirstOutput 先例）；api/panelId 面板生命周期内不变
  useEffect(() => {
    if (!ready) return;
    if (!api.isActive || !api.isGroupActive) return;
    if (!consumePanelFocusIntent(panelId)) return;
    // 延迟一帧：等 dockview overlay 渲染容器 visibility 翻开（见文件头注）
    const raf = requestAnimationFrame(() => {
      if (api.isActive && api.isGroupActive) focus();
    });
    return () => cancelAnimationFrame(raf);
  }, [api, panelId, ready]);

  // 路径②：激活事件联动（不需意图——页签点击/去重聚焦/程序化 focus 全路径）。
  // deps 同上：api/panelId 不变；ready 变化重挂订阅（闭包读最新 ready）
  useEffect(() => {
    const tryFocus = () => {
      if (ready && api.isActive && api.isGroupActive) focus();
    };
    const d1 = api.onDidActiveChange?.(() => tryFocus());
    const d2 = api.onDidActiveGroupChange?.(() => tryFocus());
    return () => {
      d1?.dispose();
      d2?.dispose();
    };
  }, [api, panelId, ready]);
}
