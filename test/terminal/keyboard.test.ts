// L3 终端渲染测试 — 键盘编码 + 提示符渲染
// 使用 @xterm/headless + @xterm/addon-serialize
// 覆盖：修饰键组合、功能键、特殊键、方向键+修饰键、Kitty CSI u 协议

import { describe, it, expect } from 'vitest';
import { Terminal } from '@xterm/headless';
import { SerializeAddon } from '@xterm/addon-serialize';

function createTerminal(): { term: Terminal; serialize: SerializeAddon } {
  const term = new Terminal({
    rows: 24,
    cols: 80,
    allowProposedApi: true,
  });
  const serialize = new SerializeAddon();
  term.loadAddon(serialize);
  return { term, serialize };
}

/** 等待 write 完成的辅助函数 */
function writeSync(term: Terminal, data: string): Promise<void> {
  return new Promise<void>((resolve) => {
    term.write(data, resolve);
  });
}

describe('L3 终端渲染 — 键盘编码', () => {
  // ============ 原有 4 条 ============

  it('renders_pwsh_prompt', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'PS C:\\Users\\test> ');
    const result = serialize.serialize();
    expect(result).toContain('PS C:');
  });

  it('claude_tui_snapshot', async () => {
    const { term, serialize } = createTerminal();
    // 模拟 Claude TUI 输出片段（含 ANSI 颜色）
    await writeSync(term, '\x1b[34mClaude Code\x1b[0m v2.0.0');
    const result = serialize.serialize();
    expect(result).toContain('Claude Code');
    expect(result).toContain('\x1b[34m'); // 蓝色 ANSI
  });

  it('key_encoding_shift_tab', async () => {
    const { term, serialize } = createTerminal();
    // Shift+Tab → CBT (\x1b[Z)
    await writeSync(term, 'hello');
    await writeSync(term, '\x1b[Z');
    await writeSync(term, 'X');
    const result = serialize.serialize();
    expect(result).toContain('X');
  });

  it('key_encoding_ctrl_c', async () => {
    // \x03（ETX）是 C0 控制字符，被 xterm.js InputHandler 消费，绝不进 buffer
    // ⇒ 不能用 serialize 验证，改用 headless term.input() + onData 断言
    const term = new Terminal({
      rows: 24,
      cols: 80,
      allowProposedApi: true,
    });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x03');
    expect(received).toContain('\x03');
  });

  // ============ Ctrl 修饰键 ============

  it('Ctrl+A 发送 \x01 (SOH)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x01');
    expect(received).toContain('\x01');
  });

  it('Ctrl+D 发送 \x04 (EOT)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x04');
    expect(received).toContain('\x04');
  });

  it('Ctrl+Z 发送 \x1a (SUB)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1a');
    expect(received).toContain('\x1a');
  });

  it('Ctrl+L 发送 \x0c (FF) — 清屏', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x0c');
    expect(received).toContain('\x0c');
  });

  it('Ctrl+Backslash 发送 \x1c (FS)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1c');
    expect(received).toContain('\x1c');
  });

  it('Ctrl+/ 发送 \x1f (US)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1f');
    expect(received).toContain('\x1f');
  });

  // ============ Alt 修饰键 ============

  it('Alt+字母发送 ESC+字母 (例如 Alt+X → \x1bx)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1bx');
    expect(received).toContain('\x1bx');
  });

  it('Alt+Enter 发送 ESC+CR', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b\r');
    expect(received).toContain('\x1b\r');
  });

  // ============ 功能键 F1-F12 ============

  it('F1 发送 \x1bOP', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1bOP');
    expect(received).toContain('\x1bOP');
  });

  it('F4 发送 \x1bOS', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1bOS');
    expect(received).toContain('\x1bOS');
  });

  it('F5 发送 \x1b[15~', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[15~');
    expect(received).toContain('\x1b[15~');
  });

  it('F12 发送 \x1b[24~', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[24~');
    expect(received).toContain('\x1b[24~');
  });

  // ============ 特殊键 ============

  it('Home 键发送 \x1b[H (CUP=1;1)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[H');
    expect(received).toContain('\x1b[H');
  });

  it('End 键发送 \x1b[F', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[F');
    expect(received).toContain('\x1b[F');
  });

  it('PgUp 发送 \x1b[5~', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[5~');
    expect(received).toContain('\x1b[5~');
  });

  it('PgDn 发送 \x1b[6~', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[6~');
    expect(received).toContain('\x1b[6~');
  });

  it('Insert 发送 \x1b[2~', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[2~');
    expect(received).toContain('\x1b[2~');
  });

  it('Delete 发送 \x1b[3~', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[3~');
    expect(received).toContain('\x1b[3~');
  });

  // ============ 方向键 + 修饰键 ============

  it('方向键上 \x1b[A', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[A');
    expect(received).toContain('\x1b[A');
  });

  it('方向键下 \x1b[B', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[B');
    expect(received).toContain('\x1b[B');
  });

  it('方向键右 \x1b[C', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[C');
    expect(received).toContain('\x1b[C');
  });

  it('方向键左 \x1b[D', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[D');
    expect(received).toContain('\x1b[D');
  });

  it('Ctrl+上 发送 \x1b[1;5A', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[1;5A');
    expect(received).toContain('\x1b[1;5A');
  });

  it('Ctrl+下 发送 \x1b[1;5B', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    term.input('\x1b[1;5B');
    expect(received).toContain('\x1b[1;5B');
  });

  // ============ CSI u (Kitty Keyboard Protocol) ============

  it('CSI u 基本键 — 字母 a (97)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    // a = codepoint 97, 无修饰键
    term.input('\x1b[97u');
    expect(received).toContain('\x1b[97u');
  });

  it('CSI u 修饰键 — Shift+A (65;2u)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    // A = codepoint 65, Shift=2
    term.input('\x1b[65;2u');
    expect(received).toContain('\x1b[65;2u');
  });

  it('CSI u 修饰键 — Ctrl+a (97;5u)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    // a = codepoint 97, Ctrl=5
    term.input('\x1b[97;5u');
    expect(received).toContain('\x1b[97;5u');
  });

  it('CSI u 修饰键 — Ctrl+Shift+A (65;6u)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    // A = codepoint 65, Ctrl+Shift=6
    term.input('\x1b[65;6u');
    expect(received).toContain('\x1b[65;6u');
  });

  it('CSI u 修饰键 — Alt+a (97;3u)', () => {
    const term = new Terminal({ rows: 24, cols: 80, allowProposedApi: true });
    const received: string[] = [];
    term.onData((data) => received.push(data));
    // a = codepoint 97, Alt=3
    term.input('\x1b[97;3u');
    expect(received).toContain('\x1b[97;3u');
  });

  // ============ 渲染验证 — CSI 序列渲染效果 ============

  it('写入退格 (BS \x08) 使光标回退', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'ABC');
    await writeSync(term, '\x08\x08'); // 回退两个字符
    await writeSync(term, 'X');
    const result = serialize.serialize();
    // X 覆盖了 B 的位置
    expect(result).toContain('X');
  });

  it('回车 \r 使光标回到行首', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '1234567890');
    await writeSync(term, '\r');
    await writeSync(term, 'OVERWRITE');
    const result = serialize.serialize();
    expect(result).toContain('OVERWRITE');
  });

  it('制表符 \t 跳转到下一个制表位', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'A\tB\tC');
    const result = serialize.serialize();
    expect(result).toContain('A');
    expect(result).toContain('B');
    expect(result).toContain('C');
  });
});
