// buildInjectedScript.ts — docViewer 预览内容注入脚本组装
//
// 由原 panels/html/HtmlPanel.tsx 的 buildInjectedScript 迁入并参数化；额外段
// （fragmentNav/linkRouter/scrollReport 等）按面板类型追加 + zoom 缩放段恒注入。
//
// 【ADR-0021 回迁主窗 DOM 后的形态】注入产物运行于「主窗内跨源沙箱 iframe
// （自定义协议宿主页）内的内容 iframe（srcdoc）」——与主窗同窗口树，消息经
// 宿主页桥 postMessage relay。keyForward 基础段恒注入（D2 收窄转发：焦点非
// 表单元素时 keydown 描述上行 → 主窗 ShortcutRegistry global context 解析；
// 表单焦点不转发——表单键入/复制快捷键解禁）。旧「基础段 keydown 转发」
//（slterm_key，主窗 dispatchEvent 重放语义）随 CP-013 退役零残留，本段不
// 复用旧名。
//
// 【拼接纪律（红线）】
// 1. 输出源码不得含 "</script>" 字面量——注入段内嵌字符串若含该字面量必须先
//    转义（宿主内容段已由 injectScript 无转义原样保留——宿主自带 </script>
//    属于其自身脚本标签的正常闭合，与注入段互不干扰，CP-031）。
// 2. 每段必须以完整语句 + 分号收尾（2026-09-06 实证：click 段原为 script 末
//    语句无分号，追加 zoom 段后同串拼接 "},true)var sltermZoom" 触发 SyntaxError
//    致整段注入脚本失效——L2 全绿仅 L4 暴露）。追加段前不必补分号（各段自带）。
// 3. 字符串插值一律经 JSON.stringify；数值常量以十进制字面量直插（iframe 内
//    独立运行，不依赖外部符号）。
// 4. postMessage targetOrigin 一律 "*"（SEC-03 实证：iframe opaque origin 下
//    targetOrigin 与 e.origin 序列化语义；宿主页桥侧 source 归属 + nonce +
//    类型白名单校验兜底——* 无额外风险）。
// 5. nonce 以 JSON.stringify 拼入（hex 无引号风险，双保险）——宿主页/主窗
//    校验 e.data.nonce === 面板挂载期随机值（SEC-04）。
//
// 段序固定：keyForward 基础段（恒首）→ extra 段 → zoom 段（恒末位）。zoom 段
// 以 "var sltermZoom=" 开头，fragmentNav 的 click 段以 "},true);" 收尾——
// html-panel 控制流断言正则 /\},true\);var sltermZoom=/ 锁死该衔接，勿调换次序。

import { buildZoomRuntimeSource } from "./zoomRuntime";
import { buildScrollRuntimeSource } from "./scrollRuntime";
import { FONT_PROBE_MSG_TYPE, KEY_FWD_MSG_TYPE } from "./previewMessages";

/** 注入到 HTML 内容中的脚本标记（幂等检测，injectScript 使用） */
export const INJECTED_MARKER = "__slterm_preview";

/**
 * keyForward 基础段函数源码（ADR-0021/D2 收窄转发，恒注入首段）——参数化
 * 匿名函数表达式（zoomRuntime/scrollRuntime 同形态）：
 *   - 生产端：buildInjectedScript 拼入，以 sltermKeyForward(document, window) 挂载；
 *   - 测试端：new Function 取回函数后在桩 doc/win 上真实执行（行为级覆盖）。
 * 语义：document keydown 捕获——焦点在表单元素（INPUT/TEXTAREA/SELECT/
 * isContentEditable）时不转发（表单键入/复制快捷键解禁）；其余场景上行按键
 * 描述（code + 修饰键，不含 key——主窗只按 code 解析）给宿主页桥 relay 主窗，
 * 主窗合成 KeyboardEvent 经 ShortcutRegistry global context 解析消费。
 * 不 preventDefault（文档内默认行为保留；global 命令集修饰组合在文档内无默认
 * 行为，不双重触发）。
 */
export function buildKeyForwardSource(nonce: string): string {
  return (
    `function(document,window){document.addEventListener("keydown",function(e){` +
    `var t=e.target;` +
    `if(t&&(t.tagName==="INPUT"||t.tagName==="TEXTAREA"||t.tagName==="SELECT"||t.isContentEditable))return;` +
    `window.parent.postMessage({type:${JSON.stringify(KEY_FWD_MSG_TYPE)},nonce:${JSON.stringify(nonce)},` +
    `code:e.code,ctrlKey:e.ctrlKey,shiftKey:e.shiftKey,altKey:e.altKey,metaKey:e.metaKey},"*");` +
    `},true);}`
  );
}

/** 面板类型可选的注入段（在 zoom 段之前追加） */
export type InjectedSegment =
  /** html：<a href="#..."> 片段链接拦截 + :target 模拟 CSS（原 HtmlPanel 第 1/3 段） */
  | { kind: "fragmentNav" }
  /** md：链接点击分类转发（http(s)/相对本地 → 上行 slterm_nav，父侧分类） */
  | { kind: "linkRouter" }
  /** md：滚动位置节流上行（iframe 重建按比例恢复） */
  | { kind: "scrollReport" }
  /** md：字体加载探针上行（E2E 专用，TE-08——宿主页 FontFaceSet 不覆盖
   *  iframe 文档，字体真实加载态只能自 iframe 内 check 后上行；仅 VITE_E2E
   *  拼装传入，生产零注入面） */
  | { kind: "fontProbe" };

/**
 * 组装注入脚本。
 *
 * zoom 段（恒注入，末位）：Ctrl+滚轮缩放——wheel capture + 下行复位/设值监听
 *   （zoomRuntime，逻辑见 zoomRuntime.ts），以 sltermZoom(document, window) 挂载
 * extra 段（面板按需传）：
 *   fragmentNav——CSS 注入（.slterm-target 基础样式）+ 片段链接拦截
 *     （WebView2 sandboxed iframe 不支持 location.hash 导航，preventDefault +
 *     class-based :target 模拟，点击时动态建 style）
 *   linkRouter / scrollReport——markdownviewer 专属（见 markdown 面板文档）
 *   fontProbe——E2E 专用字体加载探针（TE-08：仅 VITE_E2E 拼装传入，
 *     生产零注入面；iframe 内 fonts.check 后上行宿主页）
 *
 * @param nonce 面板挂载期生成的随机值，拼入 zoom/scroll/nav 消息——父侧据此
 *   校验消息来源（SEC-04；预览内容与注入脚本同文档——内容可读到 nonce，
 *   不防内容自身伪造，见下方威胁模型注记）
 *
 * 【威胁模型（2026-09 起）】nonce 明文内联于渲染文档——iframe 内任意脚本（含
 * 宿主 HTML 自带 <script>，S10-② 后真实可执行）可读取注入脚本提取 nonce 并
 * 伪造上行消息。渲染态上行（zoom/scroll/nav）伪造后果仅 HUD 百分比误导/滚动
 * 恢复偏差（低危）；keyfwd 上行（ADR-0021/D2）伪造后果 = 触发主窗 global
 * 命令（当前集合仅 global.closeTab，command-catalog 守卫锁死最小集——global
 * 集扩充时须重估本面）。SEC-04 内部伪造威胁面随 CP-012 webview 迁移消除
 * （ADR-0019），ADR-0021 回迁主窗 iframe 后内容仍经宿主页桥 relay——桥只验
 * source 归属与 origin，nonce 校验恒在主窗侧。
 */
export function buildInjectedScript(
  nonce: string,
  extra: readonly InjectedSegment[] = [],
): string {
  let out = "<script>";
  // keyForward 基础段——恒首段（ADR-0021/D2 收窄转发，逻辑见 buildKeyForwardSource）
  out += `var sltermKeyForward=(${buildKeyForwardSource(nonce)});sltermKeyForward(document,window);`;
  // nonce 的 JSON 形态（fontProbe 段经 JSON.stringify 拼入——拼接纪律 #5 双保险）
  const nonceJson = JSON.stringify(nonce);
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
    } else if (seg.kind === "fontProbe") {
      // 字体加载探针（E2E 专用，TE-08）：fonts.ready 后在 iframe 内 check
      // KaTeX_Main 真实可渲染态并上行宿主页（宿主 FontFaceSet 不覆盖 iframe）
      out +=
        `document.fonts.ready.then(function(){var ok=document.fonts.check('12px "KaTeX_Main"');` +
        `parent.postMessage({type:${JSON.stringify(FONT_PROBE_MSG_TYPE)},nonce:${nonceJson},loaded:ok},"*");});`;
    }
  }

  // zoom 运行时段——恒末位：fragmentNav 的 click 段以 "},true);" 收尾后
  // 直接衔接 "var sltermZoom="（html-panel 控制流断言正则锁死，勿调换）
  out +=
    // Ctrl+滚轮缩放运行时——匿名函数挂载，闭包状态随 iframe 文档存亡（面板会话级）
    `var sltermZoom=(${buildZoomRuntimeSource(nonce)});sltermZoom(document,window);`;

  return out + "</script>";
}
