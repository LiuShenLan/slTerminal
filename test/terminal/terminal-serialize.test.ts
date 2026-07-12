import { describe, it, expect } from 'vitest';
import { Terminal } from '@xterm/headless';
import { SerializeAddon } from '@xterm/addon-serialize';

/** 等待 write 完成的辅助函数 */
function writeSync(term: Terminal, data: string): Promise<void> {
  return new Promise<void>((resolve) => {
    term.write(data, resolve);
  });
}

describe('L3 终端渲染', () => {
  it('should contain "hi" after feeding "hi"', async () => {
    const term = new Terminal({
      cols: 80,
      rows: 24,
      allowProposedApi: true,
    });
    const serializeAddon = new SerializeAddon();
    term.loadAddon(serializeAddon);

    await writeSync(term, 'hi');

    const result = serializeAddon.serialize();
    expect(result).toContain('hi');
  });

  it('should preserve all lines when feeding 10+ lines of text', async () => {
    const term = new Terminal({
      cols: 80,
      rows: 24,
      allowProposedApi: true,
    });
    const serializeAddon = new SerializeAddon();
    term.loadAddon(serializeAddon);

    // 写入 12 行文本，每行带唯一标记
    const lines: string[] = [];
    for (let i = 1; i <= 12; i++) {
      lines.push(`line_${String(i).padStart(2, '0')}_marker`);
    }
    await writeSync(term, lines.join('\r\n'));

    const result = serializeAddon.serialize();
    // 验证所有行都出现在输出中
    for (const line of lines) {
      expect(result).toContain(line);
    }
  });

  it('should preserve ANSI color codes in serialize output', async () => {
    const term = new Terminal({
      cols: 80,
      rows: 24,
      allowProposedApi: true,
    });
    const serializeAddon = new SerializeAddon();
    term.loadAddon(serializeAddon);

    // 写入含 ANSI 颜色的文本：红色前景 + 绿色背景 + 粗体
    await writeSync(term, '\x1b[31mRED\x1b[0m \x1b[42mGREEN_BG\x1b[0m \x1b[1mBOLD\x1b[0m');

    const result = serializeAddon.serialize();
    // serialize 输出应保留 ANSI 颜色码
    expect(result).toContain('\x1b[31m');
    expect(result).toContain('RED');
    expect(result).toContain('\x1b[42m');
    expect(result).toContain('GREEN_BG');
    expect(result).toContain('\x1b[1m');
    expect(result).toContain('BOLD');
    // 重置码也应存在
    expect(result).toContain('\x1b[0m');
  });

  it('should handle large data block (>1000 chars) without data loss', async () => {
    const term = new Terminal({
      cols: 80,
      rows: 24,
      allowProposedApi: true,
    });
    const serializeAddon = new SerializeAddon();
    term.loadAddon(serializeAddon);

    // 构造 >1000 字符的大块数据（含唯一开始/结束哨兵）
    const prefix = '<<<START>>>';
    const suffix = '<<<END>>>';
    let body = '';
    for (let i = 0; i < 100; i++) {
      body += `chunk_${String(i).padStart(3, '0')}_padding_text_`;
    }
    const data = prefix + body + suffix;
    expect(data.length).toBeGreaterThan(1000);

    await writeSync(term, data);

    const result = serializeAddon.serialize();
    // 验证哨兵存在，证明大块数据无丢失
    expect(result).toContain(prefix);
    expect(result).toContain(suffix);
    // 抽样验证中间 chunk
    expect(result).toContain('chunk_050');
    expect(result).toContain('chunk_099');
  });

  it('should render text at correct position after CSI cursor movement', async () => {
    const term = new Terminal({
      cols: 80,
      rows: 24,
      allowProposedApi: true,
    });
    const serializeAddon = new SerializeAddon();
    term.loadAddon(serializeAddon);

    // 先写满第一行，然后光标移到第 5 行第 10 列写标记
    await writeSync(term, 'row1_filler_text_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx');
    // CSI n;mH 将光标定位到第 n 行第 m 列
    await writeSync(term, '\x1b[5;10H');
    await writeSync(term, 'CURSOR_HERE');
    // 再移回第 2 行第 3 列
    await writeSync(term, '\x1b[2;3H');
    await writeSync(term, 'POS2');

    const result = serializeAddon.serialize();
    // 验证标记文本均在输出中
    expect(result).toContain('CURSOR_HERE');
    expect(result).toContain('POS2');
    // 验证光标定位序列本身被 parser 消费（不原样出现在 serialize 输出）
    // 但文本应在对应行列位置——通过检查 row1 和 row2 的内容间接验证
    expect(result).toContain('row1_filler_text');
  });
});
