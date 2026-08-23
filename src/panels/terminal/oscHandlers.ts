// OSC 处理器纯注册层（TQ-E-01）——从 React hook 抽离，L3 headless 与生产 hook 共用同一真值源。
// 依赖（剪贴板/开链接/profile 匹配/会话标记）全部参数注入，本文件不 import ipc/store。
// 唯一例外（已登记）：getErrorMessage 引自 ../../lib（barrel re-export，与 useXterm 同源）——
// 纯解析函数无副作用，L3 node 环境安全（lib barrel 模块级零 DOM 访问）；FE-08 错误消息
// 统一契约（src/ipc/appError.ts 单一真值源），保持生产原行为（console.error 双参形态）。
import type { Terminal } from "@xterm/xterm";
import type { IDisposable } from "@xterm/xterm";
import { getErrorMessage } from "../../lib";

/** OSC 52 单条 payload 上限（字节，base64 前）——与 useClipboardHandler 原常量一致 */
export const MAX_OSC52_PAYLOAD = 1048576;

export interface Osc52Deps {
  /** 焦点门控：返回 false 时忽略写入（对应生产 visibleRef） */
  isVisible: () => boolean;
  writeText: (text: string) => Promise<void>;
}

/** 注册 OSC 52 剪贴板写 handler（c;base64 → 解码 → writeText） */
export function registerOsc52(term: Terminal, deps: Osc52Deps): IDisposable {
  return term.parser.registerOscHandler(52, (data: string) => {
    const semicolonIdx = data.indexOf(";");
    if (semicolonIdx === -1) return true;
    const selector = data.substring(0, semicolonIdx);
    const payload = data.substring(semicolonIdx + 1);
    // 安全策略：仅系统剪贴板（c），忽略 p（primary）和 q（secondary）
    if (selector && selector !== "c") return true;
    // 禁止读请求
    if (payload === "?" || payload.length === 0) return true;
    // Payload 上限 MAX_OSC52_PAYLOAD（1MB 防 DoS）
    if (payload.length > MAX_OSC52_PAYLOAD) return true;
    // 焦点门控：非可见面板忽略
    if (deps.isVisible() === false) return true;
    try {
      // atob 返回二进制字符串（每字符一个字节），需经 UTF-8 解码（支持 CJK 内容）
      const binary = atob(payload);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
      }
      const text = new TextDecoder().decode(bytes);
      deps.writeText(text).catch((e) => console.error("[OSC52] 写剪贴板失败:", e));
    } catch {
      // base64 非法——忽略（保持生产原行为）
    }
    return true;
  });
}

export interface Osc133Deps {
  isCommandRunning: { current: boolean };
  matchByCommand: (command: string) => { id: string; tabTitle: string } | null;
  setAgentSession: (cliId: string | null) => void;
  onTabStateChange: (state: { active: boolean; title?: string; status?: string }) => void;
}

/** 注册 OSC 133 命令边界 handler（C=开始/D=结束） */
export function registerOsc133(term: Terminal, deps: Osc133Deps): IDisposable {
  return term.parser.registerOscHandler(133, (data: string) => {
    const semicolonIndex = data.indexOf(";");
    const type = semicolonIndex >= 0 ? data.slice(0, semicolonIndex) : data;
    if (type === "C") {
      // OSC 133 C — 命令即将执行
      const command = semicolonIndex >= 0 ? data.slice(semicolonIndex + 1).trim() : "";
      const profile = deps.matchByCommand(command);
      if (profile) {
        deps.isCommandRunning.current = true;
        // B12：先写会话再发回调——TerminalPanel 的 originalTitleRef 捕获守卫
        // 检查 agentSession 非空即跳过，回调触发 onDidTitleChange 时会话必须已置位
        deps.setAgentSession(profile.id);
        deps.onTabStateChange({ active: true, title: profile.tabTitle, status: "attention" });
      }
    } else if (type === "D" && deps.isCommandRunning.current) {
      // OSC 133 D — 命令执行完毕
      deps.isCommandRunning.current = false;
      deps.onTabStateChange({ active: false });
      deps.setAgentSession(null);
    }
    // 返回 false 不消费序列，xterm.js 仍渲染提示符
    return false;
  });
}

/** OSC 8 超链接激活（openUrl 注入，失败 console.error——与生产原行为一致） */
export function makeLinkHandler(openUrl: (url: string) => Promise<void>): { activate: (event: unknown, url: string) => void } {
  return {
    activate: (_event: unknown, url: string) => {
      openUrl(url).catch((err) => console.error("打开链接失败:", getErrorMessage(err)));
    },
  };
}
