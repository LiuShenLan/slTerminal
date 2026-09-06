// zoomRuntime.ts — iframe 文档内 Ctrl+滚轮缩放的注入脚本源码生成器
//
// docViewer 预览框（PreviewFrame）的 iframe 为 opaque origin（sandbox 无
// allow-same-origin），父窗口不可触达其内部 DOM——缩放的执行逻辑必须以注入
// 脚本形式运行在 iframe 文档内。本文件把该逻辑生成为「参数化匿名函数表达式」
// 源码：
//   - 生产端：buildInjectedScript 拼入，以 sltermZoom(document, window) 挂载；
//   - 测试端：new Function 取回函数后在桩 doc/win 上真实执行——突破 jsdom
//     不执行 srcdoc iframe 脚本的缺口，使注入核心获得 L2 行为级覆盖。
//
// 语义要点：
//   1. wheel 监听挂 iframe document 捕获阶段；注入脚本经 injectScript 恒插于
//      </head> 前（先于页面 body 期脚本注册）——命中 Ctrl/⌘+滚轮时
//      preventDefault + stopImmediatePropagation：浏览器默认行为不发生、
//      同 document 后注册的页面监听一律收不到（面板接管语义）。
//   2. 无修饰键滚轮透传（页面正常滚动）；deltaMode 0/1/2 归一（像素/行/页）
//      累计到阈值才步进一档，触控板/高精度滚轮鲁棒。
//   3. 缩放状态存闭包 + documentElement.style.zoom（CSS zoom，Chromium 系
//      非标准属性但 WebView2 成立）——随 iframe 文档存亡 = 页签会话级记忆
//      （dockview always renderer 下切走切回文档存活、关页签销毁归 100%）。
//   4. 仅 zoom 实际变化才 postMessage 上行（防回声风暴）；下行复位/设值经
//      source===parent + nonce + type 三重校验。
//   5. postMessage targetOrigin 一律 "*"（2026-09-06 实证）：targetOrigin 必须匹配
//      【接收方】窗口 origin——iframe 为 opaque origin 只影响消息到达父后的
//      e.origin 序列化为 "null"，与发送 targetOrigin 无关；传 "null" 与父窗口
//      origin（http://tauri.localhost）不匹配会被 Chromium 静默丢弃。
//
// 生成纪律：插值一律 JSON.stringify（nonce 为 hex 本就无引号风险，双保险）；
// 输出源码不得含 "</script>" 字面量（injectScript 已转义宿主内容，本段自身
// 也要避免）；数值常量以十进制字面量直插，不依赖外部符号（iframe 内独立运行）。

import {
  ZOOM_MIN,
  ZOOM_MAX,
  ZOOM_STEP,
  WHEEL_STEP_PX,
  ZOOM_ROUND,
  ZOOM_MSG_TYPE,
  RESET_MSG_TYPE,
  ZOOM_SET_MSG_TYPE,
} from "./previewMessages";

/**
 * 生成缩放运行时匿名函数源码：返回的函数签名 function(doc, win)，
 * 由调用方决定挂载语义（注入脚本内 (function(){...})(document, window)）。
 *
 * @param nonce 面板挂载期随机值（SEC-04），上行/下行消息携带校验
 */
export function buildZoomRuntimeSource(nonce: string): string {
  // 常量以数字字面量直插；字符串经 JSON.stringify 引号包裹
  const min = ZOOM_MIN;
  const max = ZOOM_MAX;
  const step = ZOOM_STEP;
  const th = WHEEL_STEP_PX;
  const round = ZOOM_ROUND;
  const msgType = JSON.stringify(ZOOM_MSG_TYPE);
  const resetType = JSON.stringify(RESET_MSG_TYPE);
  const setType = JSON.stringify(ZOOM_SET_MSG_TYPE);
  const nc = JSON.stringify(nonce);

  return (
    `function(doc,win){` +
    // 闭包状态：当前缩放（1 = 100%）、delta 累计余数
    `var zoom=1,acc=0,min=${min},max=${max},step=${step},th=${th},r=${round};` +
    // 应用缩放：改写根元素 zoom + 上行上报（仅变化时由调用方保证）
    `function apply(v){zoom=v;doc.documentElement.style.zoom=String(v);` +
    `win.parent.postMessage({type:${msgType},nonce:${nc},zoom:v},"*");}` +
    // wheel 捕获——Ctrl/⌘+滚轮 = 缩放，其余透传
    `doc.addEventListener("wheel",function(e){` +
    `if(!(e.ctrlKey||e.metaKey))return;` +
    // 接管：拦截浏览器/页面默认行为，阻断同 document 后注册监听（面板接管语义）
    `e.preventDefault();e.stopImmediatePropagation();` +
    // deltaMode 归一：0=像素原样，1=行×16，2=页×th（一页视作一档）
    // 方向：上滚（deltaY<0）放大、下滚（deltaY>0）缩小（浏览器缩放惯例）
    `var d=e.deltaMode===1?e.deltaY*16:e.deltaMode===2?e.deltaY*th:e.deltaY;` +
    `acc+=d;` +
    `if(acc<=-th){acc+=th;var n=zoom*step;}else if(acc>=th){acc-=th;var n=zoom/step;}else return;` +
    // clamp + 数值整理（round6 收敛浮点尾串，同值全等比较稳定）
    `if(n>max)n=max;if(n<min)n=min;n=Math.round(n*r)/r;` +
    `if(n!==zoom)apply(n);` +
    `},true);` +
    // 下行复位/设值：source===parent + type + nonce 三重校验
    // 复位 = 归 1；设值（slterm_zoom_set，keepZoom 恢复）钳制到 [min,max] 后应用
    `win.addEventListener("message",function(e){` +
    `if(e.source!==win.parent)return;` +
    `var d=e.data;` +
    `if(!d||typeof d.type!=="string"||d.nonce!==${nc})return;` +
    `if(d.type===${resetType}){if(zoom!==1)apply(1);return;}` +
    `if(d.type!==${setType})return;` +
    `var v=d.zoom;if(typeof v!=="number"||!isFinite(v))return;` +
    `if(v>max)v=max;if(v<min)v=min;v=Math.round(v*r)/r;` +
    `if(v!==zoom)apply(v);` +
    `},false);` +
    `}`
  );
}
