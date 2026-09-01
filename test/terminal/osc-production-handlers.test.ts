// L3 终端渲染测试 — 生产 OSC 52/133/8 handler 语义（E2E-03，TQ-E-01）
// 使用 @xterm/headless 挂生产 oscHandlers.ts 注册层，断言其行为：
//   ① OSC 52 剪贴板（registerOsc52）→ mock src/ipc/clipboard.writeText + CJK 解码
//   ② OSC 133 命令边界（registerOsc133）→ onTabStateChange 参数（status/title）+ agentSession
//   ③ OSC 8 超链接（makeLinkHandler）→ mock src/ipc/shell.openUrl
//
// 实现说明（TQ-E-01 后不再复刻）：oscHandlers.ts 为纯注册层（自 React hook 抽离，依赖全
// 参数注入），L3 与生产 hook 共用同一真值源，以依赖注入注册到 headless term。依赖注入
// 形状与生产 hook 一致——matchByCommand/setAgentSession 经真实 cliProfileRegistry /
// TerminalRegistry 包一层（同 useCommandDetection 原调用形状），onTabStateChange 直接
// 接收回调。hook 包装层（useEffect/visibleRef 焦点门控的 React 侧）由 L2 use-xterm 测试覆盖。

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { Terminal } from '@xterm/headless';
import { registerOsc52, registerOsc133, makeLinkHandler } from '../../src/panels/terminal/oscHandlers';
import '../../src/features/cliProfiles/profiles'; // side-effect：注册 claude profile（首 token "claude"）
import { TerminalRegistry } from '../../src/panels/terminal/TerminalRegistry';
import { cliProfileRegistry } from '../../src/features/cliProfiles'; // 真实注册表
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

/** OSC 52 焦点门控开关（对应生产 visibleRef——React 侧由 L2 覆盖，此处仅提供注入形态） */
let visible = true;

/** 等待 write 完成的辅助函数 */
function writeSync(term: Terminal, data: string): Promise<void> {
  return new Promise<void>((resolve) => {
    term.write(data, resolve);
  });
}

function createTerminal(cols = 80, rows = 24): Terminal {
  return new Terminal({ rows, cols, allowProposedApi: true });
}

/** headless Terminal → 生产签名（@xterm/xterm）类型桥接——parser 接口两实现共用，运行时等价 */
function toXtermTerminal(term: Terminal): XtermTerminal {
  return term as unknown as XtermTerminal;
}

/** OscLinkService 内部结构（headless 6.0，无公开读接口——内部 API 断言标注） */
interface OscLinkServiceLike {
  _dataByLinkId: Map<number, { uri: string }>;
  getLinkData(linkId: number): { uri: string } | undefined;
}

describe('L3 终端渲染 — 生产 OSC handler（E2E-03）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    visible = true;
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
    registerOsc52(toXtermTerminal(term), { isVisible: () => visible, writeText });
    // base64('Hello') = SGVsbG8=
    await writeSync(term, '\x1b]52;c;SGVsbG8=\x07');
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText).toHaveBeenCalledWith('Hello');
  });

  it('OSC 52 写入 — CJK 内容 UTF-8 正确解码（atob → Uint8Array → TextDecoder）', async () => {
    const term = createTerminal();
    registerOsc52(toXtermTerminal(term), { isVisible: () => visible, writeText });
    // base64('你好') = 5L2g5aW9
    await writeSync(term, '\x1b]52;c;5L2g5aW9\x07');
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText).toHaveBeenCalledWith('你好');
  });

  it('OSC 52 读请求（payload=?）不调 writeText（仅写入策略）', async () => {
    const term = createTerminal();
    registerOsc52(toXtermTerminal(term), { isVisible: () => visible, writeText });
    await writeSync(term, '\x1b]52;c;?\x07');
    expect(writeText).not.toHaveBeenCalled();
  });

  it('OSC 52 非系统剪贴板 selector（p/q）忽略', async () => {
    const term = createTerminal();
    registerOsc52(toXtermTerminal(term), { isVisible: () => visible, writeText });
    await writeSync(term, '\x1b]52;p;SGVsbG8=\x07');
    expect(writeText).not.toHaveBeenCalled();
  });

  // ============ OSC 133 命令边界 ============

  it('OSC 133 C — 匹配 claude profile 时 onTabStateChange 携 status/title + 写 agentSession', async () => {
    const term = createTerminal();
    const states: Array<{ active: boolean; title?: string; status?: string }> = [];
    registerOsc133(toXtermTerminal(term), {
      isCommandRunning: { current: false },
      // 依赖注入形状与生产 useCommandDetection 一致（cliProfileRegistry / TerminalRegistry 真实模块）
      matchByCommand: (cmd) => cliProfileRegistry.matchByCommand(cmd),
      setAgentSession: (cliId) =>
        TerminalRegistry.setAgentSession('p1', cliId ? { cliId, matchedCommand: cliId } : null),
      onTabStateChange: (state) => states.push(state),
    });
    // shell-integration.ps1 Enter hook 发射：OSC 133 C;<命令行> ST
    await writeSync(term, '\x1b]133;C;claude --resume abc\x1b\\');
    // 首 token "claude" 命中 claude profile（tabTitle: "claude"），attention 态。
    // F9 行为修订：logo 不经此路径直传（会话绑定由 TerminalPanel 订阅驱动）
    expect(states).toEqual([
      { active: true, title: 'claude', status: 'attention' },
    ]);
    // MC-107: cliId 取匹配 profile 的 id（会话绑定 logo 数据源 + 三级解析反查键）
    expect(TerminalRegistry.get('p1')?.agentSession?.matchedCommand).toBe('claude');
    expect(TerminalRegistry.get('p1')?.agentSession?.cliId).toBe('claude');
  });

  it('OSC 133 D — 命令退出恢复 active=false + 清空 agentSession', async () => {
    const term = createTerminal();
    const states: Array<{ active: boolean; title?: string; status?: string }> = [];
    registerOsc133(toXtermTerminal(term), {
      isCommandRunning: { current: false },
      matchByCommand: (cmd) => cliProfileRegistry.matchByCommand(cmd),
      setAgentSession: (cliId) =>
        TerminalRegistry.setAgentSession('p1', cliId ? { cliId, matchedCommand: cliId } : null),
      onTabStateChange: (state) => states.push(state),
    });
    await writeSync(term, '\x1b]133;C;claude -p "hi"\x1b\\');
    // prompt() 发射 OSC 133;D;<退出码> ST
    await writeSync(term, '\x1b]133;D;0\x1b\\');
    expect(states).toEqual([
      { active: true, title: 'claude', status: 'attention' },
      { active: false },
    ]);
    expect(TerminalRegistry.get('p1')?.agentSession).toBeNull();
  });

  it('OSC 133 C — 未匹配 profile 的命令不触发 onTabStateChange / agentSession（零副作用）', async () => {
    const term = createTerminal();
    const states: Array<{ active: boolean; title?: string; status?: string }> = [];
    registerOsc133(toXtermTerminal(term), {
      isCommandRunning: { current: false },
      matchByCommand: (cmd) => cliProfileRegistry.matchByCommand(cmd),
      setAgentSession: (cliId) =>
        TerminalRegistry.setAgentSession('p1', cliId ? { cliId, matchedCommand: cliId } : null),
      onTabStateChange: (state) => states.push(state),
    });
    await writeSync(term, '\x1b]133;C;git status\x1b\\');
    expect(states).toEqual([]);
    expect(TerminalRegistry.get('p1')?.agentSession).toBeUndefined();
  });

  // ============ OSC 8 超链接 ============

  it('OSC 8 — link 解析注册 + 生产 linkHandler activate 调 openUrl', async () => {
    const term = createTerminal();
    // 生产 useXterm.ts linkHandler = makeLinkHandler(openUrl)（openUrl 经 src/ipc/shell 打开系统浏览器）。
    // headless 6.0 typings 未声明 options.linkHandler（6.1-beta 完整 xterm 才有），运行时
    // 赋值经 options setter 接受（已实测），此处经类型断言访问并标注
    const linkHandler = makeLinkHandler(openUrl);
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
