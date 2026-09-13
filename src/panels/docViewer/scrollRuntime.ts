// scrollRuntime.ts — 预览 iframe 文档内滚动上报/恢复的注入脚本源码生成器
//
// 与 zoomRuntime 同范式：md 预览 iframe（opaque origin，位于主窗内跨源沙箱
// 宿主 iframe 内嵌 srcdoc 文档，ADR-0021）的滚动状态宿主页无法直读——文档
// 滚动节流上行（slterm_scroll，120ms 防抖），iframe 重建（草稿防抖重渲染）
// 后经 slterm_scroll_set 下行按比例恢复（keepScrollRatio）。
//
// 比例语义 = scrollTop / (scrollHeight - clientHeight)，[0,1]；重建后文档高度
// 变化时按比例近似恢复（编辑点恰在视口上方时位置可能跳变——登记已知行为）。
// 下行恢复挂 setTimeout（文档 onLoad 后字体/图片加载会使 scrollHeight 变化，
// 60ms 延时提升命中率；内容异步加载完成无 scroll 事件，比例恢复为近似语义）。
//
// 拼接纪律同 zoomRuntime：无 "</script>" 字面量、语句分号收尾、targetOrigin
// "*"（仅与宿主页窗口对话，不跨窗口）、下行 source===parent + type + nonce
// 三重校验。

import { SCROLL_MSG_TYPE, SCROLL_SET_MSG_TYPE } from "./previewMessages";

/**
 * 生成滚动运行时匿名函数源码：签名 function(doc, win)（与 zoomRuntime 同挂载范式）。
 * @param nonce 面板挂载期随机值（SEC-04）
 */
export function buildScrollRuntimeSource(nonce: string): string {
  const msgType = JSON.stringify(SCROLL_MSG_TYPE);
  const setType = JSON.stringify(SCROLL_SET_MSG_TYPE);
  const nc = JSON.stringify(nonce);

  return (
    `function(doc,win){` +
    // 闭包状态：上报计时器、上次比例（等值不重发，防回声风暴）
    `var timer=null,last=-1;` +
    // 当前滚动比例（scrollTop / 可滚距离；不可滚 → -1 不上报）
    `function ratio(){var de=doc.documentElement;var max=de.scrollHeight-de.clientHeight;` +
    `if(max<=0)return -1;return de.scrollTop/max;}` +
    `function flush(){timer=null;var r=ratio();if(r<0)return;r=Math.round(r*10000)/10000;` +
    `if(r===last)return;last=r;win.parent.postMessage({type:${msgType},nonce:${nc},ratio:r},"*");}` +
    // scroll 捕获节流（120ms 防抖——拖动滚动条期间高频 scroll 事件收敛）
    `doc.addEventListener("scroll",function(){if(timer)return;timer=setTimeout(flush,120);},true);` +
    // 下行恢复：source===parent + type + nonce 三重校验；60ms 延时等文档布局收敛
    `win.addEventListener("message",function(e){` +
    `if(e.source!==win.parent)return;` +
    `var d=e.data;` +
    `if(!d||d.type!==${setType}||d.nonce!==${nc})return;` +
    `var r=Number(d.ratio);if(typeof d.ratio!=="number"||!isFinite(r)||r<0||r>1)return;` +
    `setTimeout(function(){var de=doc.documentElement;var max=de.scrollHeight-de.clientHeight;` +
    `if(max<=0)return;de.scrollTop=r*max;},60);` +
    `},false);` +
    `}`
  );
}
