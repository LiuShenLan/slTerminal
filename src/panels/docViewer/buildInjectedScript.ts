// buildInjectedScript.ts — docViewer 预览框注入脚本组装
//
// 由原 panels/html/HtmlPanel.tsx 的 buildInjectedScript 迁入并参数化：基础段
// （keydown 键盘转发）与 zoom 缩放段恒注入，额外段（fragmentNav/linkRouter/
// scrollReport 等）按面板类型追加。htmlviewer 场景注入产物与迁出前字节级
// 同构（html-panel.test.tsx 断言保真）。
//
// 【拼接纪律（红线）】
// 1. 输出源码不得含 "</script>" 字面量（injectScript 已转义宿主内容，本段自身
//    也要避免——任何子段内嵌字符串若含该字面量必须先转义）。
// 2. 每段必须以完整语句 + 分号收尾（2026-09-06 实证：click 段原为 script 末
//    语句无分号，追加 zoom 段后同串拼接 "},true)var sltermZoom" 触发 SyntaxError
//    致整段注入脚本失效——L2 全绿仅 L4 暴露）。追加段前不必补分号（各段自带）。
// 3. 字符串插值一律经 JSON.stringify；数值常量以十进制字面量直插（iframe 内
//    独立运行，不依赖外部符号）。
// 4. postMessage targetOrigin 一律 "*"（SEC-03 实证：须匹配接收方窗口 origin，
//    opaque 源只影响父侧 e.origin 序列化 "null"；父侧四层校验兜底）。
// 5. nonce 以 JSON.stringify 拼入（hex 无引号风险，双保险）——父窗口校验
//    e.data.nonce === 面板挂载期随机值（SEC-04）。
//
// 段序固定：基础段 → extra 段 → zoom 段（恒末位）。zoom 段以 "var sltermZoom="
// 开头，fragmentNav 的 click 段以 "},true);" 收尾——html-panel.test.tsx 的
// 控制流断言正则 /\},true\);var sltermZoom=/ 锁死该衔接，勿调换次序。

import { buildZoomRuntimeSource } from "./zoomRuntime";
import { buildScrollRuntimeSource } from "./scrollRuntime";

/** 注入脚本的信任标记，ShortcutRegistry 分发前可识别 postMessage 重放事件 */
export const TRUSTED_MARKER = "__slterm_postMessage";

/** 注入到 HTML 内容中的脚本标记（幂等检测，injectScript 使用） */
export const INJECTED_MARKER = "__slterm_key";

/** 面板类型可选的注入段（在基础段与 zoom 段之间追加） */
export type InjectedSegment =
  /** html：<a href="#..."> 片段链接拦截 + :target 模拟 CSS（原 HtmlPanel 第 1/3 段） */
  | { kind: "fragmentNav" }
  /** md：链接点击分类转发（http(s)/相对本地 → 父窗口，S5 引入） */
  | { kind: "linkRouter" }
  /** md：滚动位置节流上行（iframe 重建按比例恢复，S5 引入） */
  | { kind: "scrollReport" };

/**
 * 组装注入脚本。
 *
 * 基础段（恒注入）：
 *   1) 键盘转发——keydown capture → postMessage 到父窗口（消息携带面板 nonce，SEC-04）
 * zoom 段（恒注入，末位）：Ctrl+滚轮缩放——wheel capture + 下行复位/设值监听
 *   （zoomRuntime，逻辑见 zoomRuntime.ts），以 sltermZoom(document, window) 挂载
 * extra 段（面板按需传）：
 *   fragmentNav——CSS 注入（.slterm-target 基础样式）+ 片段链接拦截
 *     （WebView2 sandboxed iframe 不支持 location.hash 导航，preventDefault +
 *     class-based :target 模拟，点击时动态建 style）
 *   linkRouter / scrollReport——markdownviewer 专属（见 markdown 面板文档）
 *
 * @param nonce 面板挂载期生成的随机值，拼入 keydown/zoom postMessage——父窗口据此校验消息来源
 *
 * 【SEC-04 威胁模型（D16 登记）】nonce 明文内联于 srcDoc——iframe 内任意脚本可读取
 * 文档中的注入脚本提取 nonce 并伪造 slterm_key / slterm_zoom 消息。nonce 仅防
 * 「不知密钥的外部伪造」，不防被预览 HTML 自身。伪造 zoom 上报的后果仅为 HUD
 * 百分比误导（低危）；键盘转发由 global context 命令集最小化兜底（当前仅
 * global.closeTab 关页签，低风险）——守卫测试 command-catalog.test.ts 锁死该集合，
 * 扩充 global 命令必须先评估本威胁模型。
 */
export function buildInjectedScript(
  nonce: string,
  extra: readonly InjectedSegment[] = [],
): string {
  let out =
    "<script>" +
    // 键盘转发——postMessage targetOrigin 用 "*"（2026-09-06 实证：targetOrigin 必须匹配
    // 接收方窗口 origin；iframe 为 opaque origin 只影响消息到达父后的 e.origin 序列化，
    // 与发送 targetOrigin 无关——传 "null" 与父窗口 origin 不匹配会被 Chromium 静默丢弃，
    // 曾致键盘转发与缩放上行全灭。父侧 origin/source/nonce 校验兜底，"*" 无额外风险）
    `document.addEventListener("keydown",function(e){window.parent.postMessage({type:"slterm_key",nonce:"${nonce}",fingerprint:(e.ctrlKey?"Ctrl+":"")+(e.shiftKey?"Shift+":"")+(e.altKey?"Alt+":"")+(e.metaKey?"Meta+":"")+e.code,ctrlKey:e.ctrlKey,shiftKey:e.shiftKey,altKey:e.altKey,metaKey:e.metaKey,code:e.code,key:e.key},"*")},true);`;

  for (const seg of extra) {
    if (seg.kind === "fragmentNav") {
      out +=
        // CSS：.slterm-target 作为 :target 备选（点击时动态追加样式，保证规则
        // 在目标元素后于样式表出现亦可生效）
        `var s=document.createElement("style");s.textContent=".slterm-target{display:block!important}";document.head.appendChild(s);` +
        // 片段链接拦截 + class-based :target 模拟
        `var _h=null;document.addEventListener("click",function(e){var a=e.target.closest("a");if(!a)return;var h=a.getAttribute("href");if(!h||h.charAt(0)!=="#")return;e.preventDefault();var id=h.slice(1);` +
        // 点击 # → 清除状态
        `if(!id){if(_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target");_h=null;delete document.documentElement.dataset.sltermHash};window.scrollTo({top:0,behavior:"smooth"});return}` +
        // 同片段 → toggle
        `if(id===_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target");_h=null;delete document.documentElement.dataset.sltermHash;return}` +
        // 不同片段 → 切换
        `if(_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target")}` +
        `var el=document.getElementById(id);if(el){el.classList.add("slterm-target");el.scrollIntoView({behavior:"smooth"});document.documentElement.dataset.sltermHash=id;_h=id}` +
        `},true);`;
    } else if (seg.kind === "linkRouter") {
      // 链接点击路由（md 预览）：非 # 链接一律拦截上行 slterm_nav（href 原样——
      // http(s)/本地路径分类在父侧面板做；# 锚点在 md 渲染无目标 id，静默忽略）
      out +=
        `document.addEventListener("click",function(e){var a=e.target.closest("a");if(!a)return;var h=a.getAttribute("href");if(!h||h.charAt(0)==="#")return;` +
        `e.preventDefault();` +
        `window.parent.postMessage({type:"slterm_nav",nonce:"${nonce}",href:h},"*");` +
        `},true);`;
    } else if (seg.kind === "scrollReport") {
      // 滚动比例上报 + 下行恢复（keepScrollRatio）——scrollRuntime，逻辑见 scrollRuntime.ts
      out += `var sltermScroll=(${buildScrollRuntimeSource(nonce)});sltermScroll(document,window);`;
    }
  }

  // zoom 运行时段——恒末位：fragmentNav 的 click 段以 "},true);" 收尾后
  // 直接衔接 "var sltermZoom="（html-panel 控制流断言正则锁死，勿调换）
  out +=
    // Ctrl+滚轮缩放运行时——匿名函数挂载，闭包状态随 iframe 文档存亡（页签会话级）
    `var sltermZoom=(${buildZoomRuntimeSource(nonce)});sltermZoom(document,window);`;

  return out + "</script>";
}
