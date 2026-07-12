// L3 终端渲染测试 — OSC 序列行为
// 使用 @xterm/headless 的 onTitleChange / registerOscHandler / SerializeAddon
// 覆盖：OSC 0/2 窗口标题、OSC 4 调色板、OSC 嵌入输出完整性

import { describe, it, expect } from 'vitest';
import { Terminal } from '@xterm/headless';
import { SerializeAddon } from '@xterm/addon-serialize';

function writeSync(term: Terminal, data: string): Promise<void> {
  return new Promise<void>((resolve) => {
    term.write(data, resolve);
  });
}

function createTerminal(cols = 80, rows = 24): { term: Terminal; serialize: SerializeAddon } {
  const term = new Terminal({ cols, rows, allowProposedApi: true });
  const serialize = new SerializeAddon();
  term.loadAddon(serialize);
  return { term, serialize };
}

describe('L3 OSC 序列', () => {
  // ============ OSC 0 / OSC 2 窗口标题 ============

  it('OSC 0 BEL 终止 — onTitleChange 触发', async () => {
    const { term } = createTerminal();
    const titles: string[] = [];
    term.onTitleChange((title) => titles.push(title));

    await writeSync(term, '\x1b]0;MyTitle\x07');
    // 回读后续内容确保 write 完成
    await writeSync(term, 'AFTER');

    expect(titles).toContain('MyTitle');
  });

  it('OSC 2 BEL 终止 — onTitleChange 触发', async () => {
    const { term } = createTerminal();
    const titles: string[] = [];
    term.onTitleChange((title) => titles.push(title));

    await writeSync(term, '\x1b]2;SlTerminal\x07');
    await writeSync(term, 'AFTER');

    expect(titles).toContain('SlTerminal');
  });

  it('OSC 2 ST 终止 (ESC backslash) — 与 BEL 等效', async () => {
    const { term } = createTerminal();
    const titles: string[] = [];
    term.onTitleChange((title) => titles.push(title));

    // ST = ESC backslash (\x1b\\)
    await writeSync(term, '\x1b]2;ST_Title\x1b\\');
    await writeSync(term, 'AFTER');

    expect(titles).toContain('ST_Title');
  });

  // ============ OSC 4 调色板修改 ============

  it('OSC 4 修改调色板 — parser.registerOscHandler 收到正确数据', async () => {
    const { term } = createTerminal();
    const osc4calls: string[] = [];
    term.parser.registerOscHandler(4, (data) => {
      osc4calls.push(data);
      return true;
    });

    await writeSync(term, '\x1b]4;1;rgb:ff/00/00\x07');
    await writeSync(term, 'AFTER');

    expect(osc4calls.length).toBe(1);
    expect(osc4calls[0]).toContain('1');
    expect(osc4calls[0]).toContain('ff/00/00');
  });

  it('OSC 4 修改多个索引 — 每次独立触发 handler', async () => {
    const { term } = createTerminal();
    const osc4calls: string[] = [];
    term.parser.registerOscHandler(4, (data) => {
      osc4calls.push(data);
      return true;
    });

    // 索引 1 → 红色
    await writeSync(term, '\x1b]4;1;rgb:ff/00/00\x07');
    // 索引 4 → 蓝色
    await writeSync(term, '\x1b]4;4;rgb:00/00/ff\x07');
    await writeSync(term, 'DONE');

    expect(osc4calls.length).toBe(2);
    expect(osc4calls[0]).toContain('ff/00/00');
    expect(osc4calls[1]).toContain('00/00/ff');
  });

  // ============ OSC 嵌入在正常输出中 ============

  it('OSC 序列嵌入在正常输出中不破坏内容完整性', async () => {
    const { term, serialize } = createTerminal();

    // 正常文本 + OSC 0 设置标题 + 正常文本（嵌入式）
    await writeSync(term, 'BEFORE_\x1b]0;Embedded\x07_AFTER');

    const result = serialize.serialize();
    expect(result).toContain('BEFORE_');
    expect(result).toContain('_AFTER');
  });

  it('多个 OSC 序列穿插在文本中所有内容均保留', async () => {
    const { term, serialize } = createTerminal();

    await writeSync(term, 'A\x1b]2;T1\x07B\x1b]0;T2\x07C');
    await writeSync(term, '\r\n');

    const result = serialize.serialize();
    // OSC 序列本身不应出现在终端缓冲文本中
    expect(result).toContain('A');
    expect(result).toContain('B');
    expect(result).toContain('C');
    // 标题序列不应泄漏到屏幕内容
    expect(result).not.toContain('T1');
    expect(result).not.toContain('T2');
  });

  // ============ OSC 序列后紧跟正常文本不吞字符 ============

  it('OSC 2 后紧跟文本 — 字符不丢失', async () => {
    const { term, serialize } = createTerminal();

    // OSC 2 紧跟文本，无空格分隔
    await writeSync(term, '\x1b]2;NoGap\x07SURVIVOR');

    const result = serialize.serialize();
    expect(result).toContain('SURVIVOR');
  });

  it('OSC 4 后紧跟文本 — 字符不丢失', async () => {
    const { term, serialize } = createTerminal();

    await writeSync(term, '\x1b]4;7;rgb:aa/bb/cc\x07TRAILING');

    const result = serialize.serialize();
    expect(result).toContain('TRAILING');
  });
});
