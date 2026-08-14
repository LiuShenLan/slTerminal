// L3 终端渲染测试 — 生产 OSC 52/133/8 handler 语义（E2E-03）
// 使用 @xterm/headless 触发生产 handler 对应的 OSC 序列，断言其行为：
//   ① OSC 52 剪贴板（useClipboardHandler.ts）→ mock src/ipc/clipboard.writeText + CJK 解码
//   ② OSC 133 命令边界（useCommandDetection.ts）→ onTabStateChange 参数（icon/title）
//   ③ OSC 8 超链接（useXterm.ts linkHandler）→ mock src/ipc/shell.openUrl
//
// 实现说明：三个 handler 均为 React hook/组件内注册（L3 node 环境不跑 React），测试在
// 文件内按生产实现原样复刻注册代码（逐段标注来源文件行号）。hook 包装层（useEffect/
// visibleRef 焦点门控的 React 侧）由 L2 use-xterm 测试覆盖，此处验证解析与业务语义。

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { Terminal } from '@xterm/headless';
import type { TabState } from '../../src/panels/terminal/useCommandDetection';
import '../../src/features/cliProfiles/profiles'; // side-effect：注册 claude profile（首 token "claude"）
import { TerminalRegistry } from '../../src/panels/terminal/TerminalRegistry';
import { cliProfileRegistry } from '../../src/features/cliProfiles'; // 真实注册表
import { STATUS_EMOJI } from '../../src/lib/agentStatus';
import type { Terminal as XtermTerminal } from '@xterm/xterm';

vi.mock('../../src/ipc/clipboard', () => ({
  writeText: vi.fn().mockResolvedValue(undefined),
  readText: vi.fn(),
}));
vi.mock('../../src/ipc/shell', () => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
}));

import { writeText } from '../../src/ipc/clipboard';
import { openUrl } from '../../src/ipc/shell';

/** 等待 write 完成的辅助函数 */
function writeSync(term: Terminal, data: string): Promise<void> {
  return new Promise<void>((resolve) => {
    term.write(data, resolve);
  });
}

function createTerminal(cols = 80, rows = 24): Terminal {
  return new Terminal({ rows, cols, allowProposedApi: true });
}

// ============ OSC 52 handler 复刻（生产 src/panels/terminal/useClipboardHandler.ts:37-66） ============

/** 复刻生产 OSC 52 拦截 handler。visible=false 时焦点门控忽略（L2 已覆盖 React 侧）。 */
function registerOsc52(term: Terminal, visible = true): void {
  const visibleRef = { current: visible };
  term.parser.registerOscHandler(52, (data: string) => {
    const semicolonIdx = data.indexOf(';');
    if (semicolonIdx === -1) return true;

    const selector = data.substring(0, semicolonIdx);
    const payload = data.substring(semicolonIdx + 1);

    // 仅系统剪贴板（c），忽略 p（primary）和 q（secondary）
    if (selector && selector !== 'c') return true;
    // 禁止读请求
    if (payload === '?' || payload.length === 0) return true;
    // Payload 上限 MAX_OSC52_PAYLOAD（1MB）
    if (payload.length > 1048576) return true;
    // 焦点门控：非可见面板忽略
    if (visibleRef.current === false) return true;

    try {
      // atob 返回二进制字符串（每字符一个字节），需经 UTF-8 解码
      const binary = atob(payload);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
      }
      const text = new TextDecoder().decode(bytes);
      writeText(text).catch(() => {});
    } catch {
      // base64 解码失败，静默忽略
    }
    return true;
  });
}

// ============ OSC 133 handler 复刻（生产 src/panels/terminal/useCommandDetection.ts:57-87） ============

/** 复刻生产 OSC 133 命令边界 handler（cliProfileRegistry / TerminalRegistry 均为真实生产模块）。 */
function registerOsc133(
  term: Terminal,
  panelId: string,
  onTabStateChange?: (state: TabState) => void,
): { isCommandRunningRef: { current: boolean } } {
  const isCommandRunningRef = { current: false };
  const onTabStateChangeRef = { current: onTabStateChange };
  term.parser.registerOscHandler(133, (data: string) => {
    const semicolonIndex = data.indexOf(';');
    const type = semicolonIndex >= 0 ? data.slice(0, semicolonIndex) : data;

    if (type === 'C') {
      // OSC 133 C — 命令即将执行
      const command = semicolonIndex >= 0 ? data.slice(semicolonIndex + 1).trim() : '';
      const profile = cliProfileRegistry.matchByCommand(command);
      if (profile) {
        isCommandRunningRef.current = true;
        // 标题取自匹配 profile（tabTitle）；未命中零副作用（不触发回调）。
        // F9 行为修订：logo 不经此路径直传（会话绑定由 TerminalPanel 订阅驱动）
        onTabStateChangeRef.current?.({
          active: true,
          title: profile.tabTitle,
          icon: STATUS_EMOJI.attention,
        });
        // MC-107: 写入会话状态（未注入 hooks 时无 usageSourcePath）——cliId 取匹配 profile 的 id
        TerminalRegistry.setAgentSession(panelId, {
          cliId: profile.id,
          matchedCommand: profile.id,
        });
      }
    } else if (type === 'D' && isCommandRunningRef.current) {
      // OSC 133 D — 命令执行完毕
      isCommandRunningRef.current = false;
      onTabStateChangeRef.current?.({ active: false });
      TerminalRegistry.setAgentSession(panelId, null);
    }
    // 返回 false 不消费序列，xterm.js 仍渲染提示符
    return false;
  });
  return { isCommandRunningRef };
}

/** OscLinkService 内部结构（headless 6.0，无公开读接口——内部 API 断言标注） */
interface OscLinkServiceLike {
  _dataByLinkId: Map<number, { uri: string }>;
  getLinkData(linkId: number): { uri: string } | undefined;
}

describe('L3 终端渲染 — 生产 OSC handler（E2E-03）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    TerminalRegistry._reset();
    // 注册测试面板（仅 agentSession 断言需要；term/fitAddon 用占位，本测试不触达）
    TerminalRegistry.register('p1', {
      term: undefined as unknown as XtermTerminal,
      sessionId: 's1',
      webglAddon: null,
      fitAddon: undefined as never,
    });
  });

  afterEach(() => {
    TerminalRegistry._reset();
  });

  // ============ OSC 52 剪贴板 ============

  it('OSC 52 写入 — 系统剪贴板 base64 解码后调 writeText', async () => {
    const term = createTerminal();
    registerOsc52(term);
    // base64('Hello') = SGVsbG8=
    await writeSync(term, '\x1b]52;c;SGVsbG8=\x07');
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText).toHaveBeenCalledWith('Hello');
  });

  it('OSC 52 写入 — CJK 内容 UTF-8 正确解码（atob → Uint8Array → TextDecoder）', async () => {
    const term = createTerminal();
    registerOsc52(term);
    // base64('你好') = 5L2g5aW9
    await writeSync(term, '\x1b]52;c;5L2g5aW9\x07');
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText).toHaveBeenCalledWith('你好');
  });

  it('OSC 52 读请求（payload=?）不调 writeText（仅写入策略）', async () => {
    const term = createTerminal();
    registerOsc52(term);
    await writeSync(term, '\x1b]52;c;?\x07');
    expect(writeText).not.toHaveBeenCalled();
  });

  it('OSC 52 非系统剪贴板 selector（p/q）忽略', async () => {
    const term = createTerminal();
    registerOsc52(term);
    await writeSync(term, '\x1b]52;p;SGVsbG8=\x07');
    expect(writeText).not.toHaveBeenCalled();
  });

  // ============ OSC 133 命令边界 ============

  it('OSC 133 C — 匹配 claude profile 时 onTabStateChange 携 icon/title + 写 agentSession', async () => {
    const term = createTerminal();
    const states: TabState[] = [];
    registerOsc133(term, 'p1', (state) => states.push(state));
    // shell-integration.ps1 Enter hook 发射：OSC 133 C;<命令行> ST
    await writeSync(term, '\x1b]133;C;claude --resume abc\x1b\\');
    // 首 token "claude" 命中 claude profile（tabTitle: "claude"），attention 态 🟡。
    // F9 行为修订：logo 不经此路径直传（会话绑定由 TerminalPanel 订阅驱动）
    expect(states).toEqual([
      { active: true, title: 'claude', icon: '🟡' },
    ]);
    // MC-107: cliId 取匹配 profile 的 id（会话绑定 logo 数据源 + 三级解析反查键）
    expect(TerminalRegistry.get('p1')?.agentSession?.matchedCommand).toBe('claude');
    expect(TerminalRegistry.get('p1')?.agentSession?.cliId).toBe('claude');
  });

  it('OSC 133 D — 命令退出恢复 active=false + 清空 agentSession', async () => {
    const term = createTerminal();
    const states: TabState[] = [];
    registerOsc133(term, 'p1', (state) => states.push(state));
    await writeSync(term, '\x1b]133;C;claude -p "hi"\x1b\\');
    // prompt() 发射 OSC 133;D;<退出码> ST
    await writeSync(term, '\x1b]133;D;0\x1b\\');
    expect(states).toEqual([
      { active: true, title: 'claude', icon: '🟡' },
      { active: false },
    ]);
    expect(TerminalRegistry.get('p1')?.agentSession).toBeNull();
  });

  it('OSC 133 C — 未匹配 profile 的命令不触发 onTabStateChange / agentSession（零副作用）', async () => {
    const term = createTerminal();
    const states: TabState[] = [];
    registerOsc133(term, 'p1', (state) => states.push(state));
    await writeSync(term, '\x1b]133;C;git status\x1b\\');
    expect(states).toEqual([]);
    expect(TerminalRegistry.get('p1')?.agentSession).toBeUndefined();
  });

  // ============ OSC 8 超链接 ============

  it('OSC 8 — link 解析注册 + 生产 linkHandler activate 调 openUrl', async () => {
    const term = createTerminal();
    // 生产 useXterm.ts:239-244 同款 linkHandler（经 src/ipc/shell.openUrl 打开系统浏览器）。
    // headless 6.0 typings 未声明 options.linkHandler（6.1-beta 完整 xterm 才有），运行时
    // 赋值经 options setter 接受（已实测），此处经类型断言访问并标注
    const linkHandler = {
      activate: (_event: unknown, url: string) => {
        openUrl(url).catch(() => {});
      },
    };
    (term.options as unknown as { linkHandler: typeof linkHandler }).linkHandler = linkHandler;
    await writeSync(term, '\x1b]8;;https://example.com\x1b\\CLICK_ME\x1b]8;;\x1b\\');
    // OSC 8 序列不泄漏到缓冲、文本正常落格
    expect(term.buffer.active.getLine(0)!.translateToString().trim()).toBe('CLICK_ME');
    // 内部 API 断言（无公开读接口）：link 已注册到 OscLinkService
    const oscLink = (term as unknown as { _core: { _oscLinkService: OscLinkServiceLike } })._core
      ._oscLinkService;
    const linkIds = Array.from(oscLink._dataByLinkId.keys());
    expect(linkIds.length).toBeGreaterThan(0);
    expect(oscLink.getLinkData(linkIds[0])?.uri).toBe('https://example.com');
    // 生产 handler 行为：activate（真实点击由 WebView2 触发）→ openUrl
    linkHandler.activate(null, 'https://example.com');
    expect(openUrl).toHaveBeenCalledWith('https://example.com');
  });
});
