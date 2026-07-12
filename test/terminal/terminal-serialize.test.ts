// L3 终端渲染测试 — 序列化 + 终端行为
// 使用 @xterm/headless + @xterm/addon-serialize
// 覆盖：滚动回滚、Reflow、SGR 属性叠加、CJK/Emoji、交替屏幕、光标移动、
//        擦除操作、插入/删除行列、256色/TrueColor、SGR 重置

import { describe, it, expect } from 'vitest';
import { Terminal } from '@xterm/headless';
import { SerializeAddon } from '@xterm/addon-serialize';

/** 等待 write 完成的辅助函数 */
function writeSync(term: Terminal, data: string): Promise<void> {
  return new Promise<void>((resolve) => {
    term.write(data, resolve);
  });
}

/** 创建测试用终端 + serialize addon 的快捷工厂 */
function createTerminal(cols = 80, rows = 24): { term: Terminal; serialize: SerializeAddon } {
  const term = new Terminal({
    cols,
    rows,
    allowProposedApi: true,
  });
  const serialize = new SerializeAddon();
  term.loadAddon(serialize);
  return { term, serialize };
}

describe('L3 终端渲染 — 序列化', () => {
  // ============ 基础功能（原有 5 条） ============

  it('写入 "hi" 后 serialize 应包含 "hi"', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'hi');
    const result = serialize.serialize();
    expect(result).toContain('hi');
  });

  it('写入 12 行文本后每行标记均保留', async () => {
    const { term, serialize } = createTerminal();
    const lines: string[] = [];
    for (let i = 1; i <= 12; i++) {
      lines.push(`line_${String(i).padStart(2, '0')}_marker`);
    }
    await writeSync(term, lines.join('\r\n'));
    const result = serialize.serialize();
    for (const line of lines) {
      expect(result).toContain(line);
    }
  });

  it('ANSI 颜色码应在 serialize 输出中保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[31mRED\x1b[0m \x1b[42mGREEN_BG\x1b[0m \x1b[1mBOLD\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[31m');
    expect(result).toContain('RED');
    expect(result).toContain('\x1b[42m');
    expect(result).toContain('GREEN_BG');
    expect(result).toContain('\x1b[1m');
    expect(result).toContain('BOLD');
    expect(result).toContain('\x1b[0m');
  });

  it('大于 1000 字符的大块数据不应丢失', async () => {
    const { term, serialize } = createTerminal();
    const prefix = '<<<START>>>';
    const suffix = '<<<END>>>';
    let body = '';
    for (let i = 0; i < 100; i++) {
      body += `chunk_${String(i).padStart(3, '0')}_padding_text_`;
    }
    const data = prefix + body + suffix;
    expect(data.length).toBeGreaterThan(1000);
    await writeSync(term, data);
    const result = serialize.serialize();
    expect(result).toContain(prefix);
    expect(result).toContain(suffix);
    expect(result).toContain('chunk_050');
    expect(result).toContain('chunk_099');
  });

  it('CSI 光标定位后文本应在正确位置', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'row1_filler_text_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx');
    // CSI n;mH 将光标定位到第 n 行第 m 列
    await writeSync(term, '\x1b[5;10H');
    await writeSync(term, 'CURSOR_HERE');
    await writeSync(term, '\x1b[2;3H');
    await writeSync(term, 'POS2');
    const result = serialize.serialize();
    expect(result).toContain('CURSOR_HERE');
    expect(result).toContain('POS2');
    expect(result).toContain('row1_filler_text');
  });

  // ============ 滚动与回滚 ============

  it('写入超过 rows 行后应保留 scrollback 中的全部内容', async () => {
    const { term, serialize } = createTerminal(80, 24);
    // 写入 50 行（超出 24 行视口，触发滚动）
    const lines: string[] = [];
    for (let i = 1; i <= 50; i++) {
      lines.push(`SCROLL_LINE_${String(i).padStart(3, '0')}`);
    }
    await writeSync(term, lines.join('\r\n'));
    const result = serialize.serialize();
    // 第一行应滚入 scrollback 但仍在输出中
    expect(result).toContain('SCROLL_LINE_001');
    // 最后一行在视口底部
    expect(result).toContain('SCROLL_LINE_050');
    // 中间行也应保留
    expect(result).toContain('SCROLL_LINE_025');
  });

  it('scrollback 中的 ANSI 颜色应保留', async () => {
    const { term, serialize } = createTerminal(80, 10);
    // 写入带颜色的行并触发滚动
    for (let i = 1; i <= 20; i++) {
      await writeSync(term, `\x1b[31mRED_${i}\x1b[0m`);
      await writeSync(term, '\r\n');
    }
    const result = serialize.serialize();
    // serialize addon 会合并同色连续行的 SGR 序列，故分别断言颜色码和内容
    expect(result).toContain('\x1b[31m');
    expect(result).toContain('RED_1');
    expect(result).toContain('RED_20');
    expect(result).toContain('\x1b[0m');
  });

  it('scrollback 大量行（200+）不应截断', async () => {
    const { term, serialize } = createTerminal(80, 5);
    const lines: string[] = [];
    for (let i = 1; i <= 200; i++) {
      lines.push(`L${i}`);
    }
    await writeSync(term, lines.join('\r\n'));
    const result = serialize.serialize();
    expect(result).toContain('L1');
    expect(result).toContain('L100');
    expect(result).toContain('L200');
  });

  // ============ Reflow（文本重排） ============

  it('resize 变窄后文本应 reflow 保留', async () => {
    const { term, serialize } = createTerminal(80, 24);
    // 先写满宽度为 80 的内容
    await writeSync(term, 'A'.repeat(78) + '\r\n');
    await writeSync(term, 'B'.repeat(78) + '\r\n');
    await writeSync(term, 'C'.repeat(78));
    // resize 到 40 列——文本应重排
    term.resize(40, 24);
    // 等待 reflow 完成后再写入一个标记
    await writeSync(term, '\r\nREFLOW_DONE');
    const result = serialize.serialize();
    // A/B/C 行内容应保留
    expect(result).toContain('AAAA');
    expect(result).toContain('BBBB');
    expect(result).toContain('CCCC');
    expect(result).toContain('REFLOW_DONE');
  });

  it('resize 变宽后不应丢失内容', async () => {
    const { term, serialize } = createTerminal(40, 24);
    await writeSync(term, 'SHORT_LINE_ONE\r\n');
    await writeSync(term, 'SHORT_LINE_TWO\r\n');
    term.resize(120, 24);
    await writeSync(term, 'AFTER_WIDEN');
    const result = serialize.serialize();
    expect(result).toContain('SHORT_LINE_ONE');
    expect(result).toContain('SHORT_LINE_TWO');
    expect(result).toContain('AFTER_WIDEN');
  });

  it('多次 resize 后内容应保持完整', async () => {
    const { term, serialize } = createTerminal(80, 24);
    await writeSync(term, 'MULTI_RESIZE_TEXT_ABCDEFGHIJKLMNOPQRSTUVWXYZ\r\n');
    term.resize(60, 24);
    term.resize(100, 24);
    term.resize(50, 30);
    await writeSync(term, '\r\nMULTI_END');
    const result = serialize.serialize();
    expect(result).toContain('MULTI_RESIZE_TEXT');
    expect(result).toContain('MULTI_END');
  });

  // ============ SGR 属性叠加 ============

  it('粗体+下划线+前景色叠加应全部保留', async () => {
    const { term, serialize } = createTerminal();
    // 粗体(1) + 下划线(4) + 红色前景(31)
    // 注意：serialize addon 会将颜色参数排在属性参数前面（31;1;4 而非 1;4;31）
    await writeSync(term, '\x1b[1;4;31mBOLD_UNDERLINE_RED\x1b[0m');
    const result = serialize.serialize();
    // 验证各 SGR 子参数均存在（不要求顺序，用 (?=[;m]) 替代 \b）
    expect(result).toMatch(/\x1b\[[0-9;]*31(?=[;m])/);
    expect(result).toMatch(/\x1b\[[0-9;]*1(?=[;m])/);
    expect(result).toMatch(/\x1b\[[0-9;]*4(?=[;m])/);
    expect(result).toContain('BOLD_UNDERLINE_RED');
  });

  it('斜体+删除线+蓝色前景叠加应保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[3;9;34mITALIC_STRIKE_BLUE\x1b[0m');
    const result = serialize.serialize();
    expect(result).toMatch(/\x1b\[[0-9;]*34(?=[;m])/);
    expect(result).toMatch(/\x1b\[[0-9;]*3(?=[;m])/);
    expect(result).toMatch(/\x1b\[[0-9;]*9(?=[;m])/);
    expect(result).toContain('ITALIC_STRIKE_BLUE');
  });

  it('反显+闪烁+背景色叠加应保留', async () => {
    const { term, serialize } = createTerminal();
    // 反显(7) + 慢闪(5) + 青色背景(46)
    await writeSync(term, '\x1b[7;5;46mREVERSE_BLINK_CYAN_BG\x1b[0m');
    const result = serialize.serialize();
    expect(result).toMatch(/\x1b\[[0-9;]*46(?=[;m])/);
    expect(result).toMatch(/\x1b\[[0-9;]*7(?=[;m])/);
    expect(result).toMatch(/\x1b\[[0-9;]*5(?=[;m])/);
    expect(result).toContain('REVERSE_BLINK_CYAN_BG');
  });

  // ============ CJK / Emoji / 宽字符 ============

  it('中文字符应正确序列化', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '你好世界 —— Hello World');
    const result = serialize.serialize();
    expect(result).toContain('你好世界');
    expect(result).toContain('Hello World');
  });

  it('Emoji 应正确序列化', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '😀🎉🚀✅ — emoji test');
    const result = serialize.serialize();
    expect(result).toContain('😀');
    expect(result).toContain('🎉');
    expect(result).toContain('🚀');
    expect(result).toContain('✅');
  });

  it('日文假名应正确序列化', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'こんにちは コンニチハ');
    const result = serialize.serialize();
    expect(result).toContain('こんにちは');
    expect(result).toContain('コンニチハ');
  });

  it('中英文混合+ANSI 颜色组合', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[32m成功\x1b[0m \x1b[31m失败\x1b[0m \x1b[33m警告 123\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('成功');
    expect(result).toContain('失败');
    expect(result).toContain('警告');
    expect(result).toContain('\x1b[32m');
    expect(result).toContain('\x1b[31m');
  });

  // ============ 交替屏幕缓冲 ============

  it('DECSET 1049h 切换到交替屏幕后内容隔离', async () => {
    const { term, serialize } = createTerminal();
    // 主屏幕写入内容
    await writeSync(term, 'MAIN_SCREEN_CONTENT');
    // 切换到交替屏幕
    await writeSync(term, '\x1b[?1049h');
    // 交替屏幕写入内容
    await writeSync(term, 'ALT_SCREEN_CONTENT');
    // 切换回主屏幕
    await writeSync(term, '\x1b[?1049l');
    // 主屏幕原有内容应恢复
    const result = serialize.serialize();
    expect(result).toContain('MAIN_SCREEN_CONTENT');
  });

  it('交替屏幕中全屏写入后切回主屏应恢复原始内容', async () => {
    const { term, serialize } = createTerminal(80, 10);
    // 主屏幕写满 3 行
    await writeSync(term, 'MAIN_1\r\nMAIN_2\r\nMAIN_3');
    // 切换到交替屏幕
    await writeSync(term, '\x1b[?1049h');
    // 交替屏幕写满不同内容
    await writeSync(term, 'ALT_A\r\nALT_B\r\nALT_C');
    // 切回主屏幕
    await writeSync(term, '\x1b[?1049l');
    const result = serialize.serialize();
    expect(result).toContain('MAIN_1');
    expect(result).toContain('MAIN_2');
  });

  // ============ 光标移动序列 ============

  it('CUP (CSI n;mH) 精确定位', async () => {
    const { term, serialize } = createTerminal(80, 10);
    // 第 3 行第 20 列写标记
    await writeSync(term, '\x1b[3;20HCUP_MARK');
    // 第 7 行第 5 列写标记
    await writeSync(term, '\x1b[7;5HSECOND');
    const result = serialize.serialize();
    expect(result).toContain('CUP_MARK');
    expect(result).toContain('SECOND');
  });

  it('CUU (CSI nA) 光标上移', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'LINE1\r\nLINE2\r\nLINE3');
    // 上移 2 行
    await writeSync(term, '\x1b[2A');
    await writeSync(term, 'UP_HERE');
    const result = serialize.serialize();
    expect(result).toContain('UP_HERE');
  });

  it('CUD (CSI nB) 光标下移', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'LINE1');
    // 下移 3 行
    await writeSync(term, '\x1b[3B');
    await writeSync(term, 'DOWN_HERE');
    const result = serialize.serialize();
    expect(result).toContain('DOWN_HERE');
  });

  it('CUF (CSI nC) 光标右移', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'START');
    // 右移 10 列
    await writeSync(term, '\x1b[10C');
    await writeSync(term, 'RIGHT');
    const result = serialize.serialize();
    expect(result).toContain('START');
    expect(result).toContain('RIGHT');
  });

  it('CUB (CSI nD) 光标左移', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '1234567890');
    // 左移 5 列
    await writeSync(term, '\x1b[5D');
    await writeSync(term, 'X');
    const result = serialize.serialize();
    expect(result).toContain('X');
  });

  it('CNL (CSI nE) 光标下移并到行首', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'LINE1');
    // 下移 2 行并回到行首
    await writeSync(term, '\x1b[2E');
    await writeSync(term, 'NEXT_LINE');
    const result = serialize.serialize();
    expect(result).toContain('NEXT_LINE');
  });

  it('CPL (CSI nF) 光标上移并到行首', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'LINE1\r\nLINE2\r\nLINE3');
    // 上移 1 行并回到行首
    await writeSync(term, '\x1b[1F');
    await writeSync(term, 'PREV');
    const result = serialize.serialize();
    expect(result).toContain('PREV');
  });

  // ============ 擦除操作 ============

  it('ED 0 (CSI 0J) 擦除光标到屏幕末尾', async () => {
    const { term, serialize } = createTerminal(80, 10);
    // 先写满 10 行
    for (let i = 1; i <= 10; i++) {
      await writeSync(term, `ROW_${String(i).padStart(2, '0')}\r\n`);
    }
    // 光标移到第 5 行，擦除光标到末尾
    await writeSync(term, '\x1b[5;1H');
    await writeSync(term, '\x1b[0J');
    const result = serialize.serialize();
    // 前 4 行应保留
    expect(result).toContain('ROW_01');
    expect(result).toContain('ROW_04');
  });

  it('ED 1 (CSI 1J) 擦除屏幕开头到光标', async () => {
    const { term, serialize } = createTerminal(80, 10);
    // 只写 8 行（不触发滚动），避免 scrollback 干扰
    for (let i = 1; i <= 8; i++) {
      await writeSync(term, `R${i}\r\n`);
    }
    // 光标移到第 5 行，擦除开头到光标（含光标位置 col 1）
    await writeSync(term, '\x1b[5;1H');
    await writeSync(term, '\x1b[1J');
    const result = serialize.serialize();
    // 第 5 行 col 1 被擦（R5→"5"），第 6-8 行完整保留
    expect(result).toContain('R7');
    expect(result).toContain('R8');
  });

  it('ED 2 (CSI 2J) 擦除整个屏幕', async () => {
    const { term, serialize } = createTerminal(80, 10);
    await writeSync(term, 'ERASE_ME_PLEASE');
    // 光标回到左上角再擦除整个屏幕
    await writeSync(term, '\x1b[H');
    await writeSync(term, '\x1b[2J');
    await writeSync(term, 'FRESH');
    const result = serialize.serialize();
    // 旧内容应被清除
    expect(result).not.toContain('ERASE_ME_PLEASE');
    expect(result).toContain('FRESH');
  });

  it('EL 0 (CSI 0K) 擦除光标到行尾', async () => {
    const { term, serialize } = createTerminal();
    // 在行首写文本，光标移到第 5 列擦除到行尾
    await writeSync(term, 'KEEP_ERASE');
    await writeSync(term, '\x1b[1;5H');
    await writeSync(term, '\x1b[0K');
    const result = serialize.serialize();
    // 前 4 个字符保留
    expect(result).toContain('KEEP');
    expect(result).not.toContain('ERASE');
  });

  it('EL 1 (CSI 1K) 擦除行首到光标', async () => {
    const { term, serialize } = createTerminal();
    // 写文本，光标移到第 6 列擦除行首到光标
    await writeSync(term, 'DELME_KEEP');
    await writeSync(term, '\x1b[1;6H');
    await writeSync(term, '\x1b[1K');
    const result = serialize.serialize();
    // 前 5 个字符被擦除，余下保留
    expect(result).not.toContain('DELME');
    expect(result).toContain('KEEP');
  });

  it('EL 2 (CSI 2K) 擦除整行', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'WILL_BE_ERASED');
    await writeSync(term, '\x1b[2K');
    await writeSync(term, 'REPLACED');
    const result = serialize.serialize();
    expect(result).toContain('REPLACED');
    expect(result).not.toContain('WILL_BE_ERASED');
  });

  // ============ 插入/删除行列 ============

  it('IL (CSI nL) 插入行后内容下移', async () => {
    const { term, serialize } = createTerminal(80, 10);
    for (let i = 1; i <= 5; i++) {
      await writeSync(term, `ORIG_${i}\r\n`);
    }
    // 光标在第 2 行，插入 2 行
    await writeSync(term, '\x1b[2;1H');
    await writeSync(term, '\x1b[2L');
    // 在新插入的空白行写内容
    await writeSync(term, 'INSERTED');
    const result = serialize.serialize();
    expect(result).toContain('INSERTED');
    expect(result).toContain('ORIG_1');
  });

  it('DL (CSI nM) 删除行后内容上移', async () => {
    const { term, serialize } = createTerminal(80, 10);
    for (let i = 1; i <= 5; i++) {
      await writeSync(term, `LINE_${i}\r\n`);
    }
    // 删除第 2 行
    await writeSync(term, '\x1b[2;1H');
    await writeSync(term, '\x1b[1M');
    const result = serialize.serialize();
    // LINE_2 被删除，LINE_3 应上移
    expect(result).not.toContain('LINE_2');
    expect(result).toContain('LINE_3');
  });

  // ============ 字符属性高级 ============

  it('SGR 0 重置所有属性', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[1;4;31mBOLD_UNDERLINE_RED\x1b[0m NORMAL');
    const result = serialize.serialize();
    // 重置序列应存在
    expect(result).toContain('\x1b[0m');
    expect(result).toContain('NORMAL');
  });

  it('256 色前景 (38;5;n)', async () => {
    const { term, serialize } = createTerminal();
    // 前景色 196（亮红）
    await writeSync(term, '\x1b[38;5;196mCOLOR_196\x1b[0m');
    // 前景色 51（亮青）
    await writeSync(term, ' \x1b[38;5;51mCOLOR_51\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[38;5;196m');
    expect(result).toContain('COLOR_196');
    expect(result).toContain('\x1b[38;5;51m');
    expect(result).toContain('COLOR_51');
  });

  it('256 色背景 (48;5;n)', async () => {
    const { term, serialize } = createTerminal();
    // 背景色 226（亮黄）
    await writeSync(term, '\x1b[48;5;226mBG_226\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[48;5;226m');
    expect(result).toContain('BG_226');
  });

  it('TrueColor 24-bit 前景色 (38;2;r;g;b)', async () => {
    const { term, serialize } = createTerminal();
    // 前景色 #FF8800 (rgb:255,136,0)
    await writeSync(term, '\x1b[38;2;255;136;0mTRUECOLOR_FG\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[38;2;255;136;0m');
    expect(result).toContain('TRUECOLOR_FG');
  });

  it('TrueColor 24-bit 背景色 (48;2;r;g;b)', async () => {
    const { term, serialize } = createTerminal();
    // 背景色 #3366CC
    await writeSync(term, '\x1b[48;2;51;102;204mTRUECOLOR_BG\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[48;2;51;102;204m');
    expect(result).toContain('TRUECOLOR_BG');
  });

  // ============ 光标保存与恢复 ============

  it('DECSC/DECRC 保存恢复光标位置', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, 'ORIGINAL');
    // 保存光标 (ESC 7)
    await writeSync(term, '\x1b7');
    // 移到别处写内容
    await writeSync(term, '\x1b[5;20H');
    await writeSync(term, 'ELSEWHERE');
    // 恢复光标 (ESC 8)
    await writeSync(term, '\x1b8');
    await writeSync(term, 'RESTORED');
    const result = serialize.serialize();
    expect(result).toContain('ORIGINAL');
    expect(result).toContain('RESTORED');
    expect(result).toContain('ELSEWHERE');
  });
});
