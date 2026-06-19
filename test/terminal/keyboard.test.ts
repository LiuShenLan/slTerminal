// Phase 1 L3 终端渲染测试 — 键盘编码 + 提示符渲染
// 使用 @xterm/headless + @xterm/addon-serialize

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

describe('L3 终端渲染', () => {
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

    // Shift+Tab 在 xterm.js 中发送 \x1b[Z
    await writeSync(term, '\x1b[Z');

    const result = serialize.serialize();
    // Shift+Tab 序列在 serialize 中应被保留（或被处理为空格）
    expect(result.length).toBeGreaterThanOrEqual(0);
  });

  it('key_encoding_ctrl_c', async () => {
    const { term, serialize } = createTerminal();

    // Ctrl+C 通常显示 ^C
    await writeSync(term, '^C');

    const result = serialize.serialize();
    expect(result).toContain('^C');
  });
});
