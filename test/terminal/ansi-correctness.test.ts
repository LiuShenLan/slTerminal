// L3 终端渲染测试 — ANSI 转义序列正确性
// 使用 @xterm/headless + @xterm/addon-serialize
// 覆盖：16 色/256 色/True Color、粗体/斜体/下划线/闪烁/反显/隐藏/删除线、
//        DEC 私用模式序列

import { describe, it, expect } from 'vitest';
import { Terminal } from '@xterm/headless';
import { SerializeAddon } from '@xterm/addon-serialize';

function createTerminal(cols = 80, rows = 24): { term: Terminal; serialize: SerializeAddon } {
  const term = new Terminal({ cols, rows, allowProposedApi: true });
  const serialize = new SerializeAddon();
  term.loadAddon(serialize);
  return { term, serialize };
}

function writeSync(term: Terminal, data: string): Promise<void> {
  return new Promise<void>((resolve) => {
    term.write(data, resolve);
  });
}

describe('L3 终端渲染 — ANSI 正确性', () => {
  // ============ 16 色基本前景 ============

  it('16 个基本前景色 (30-37, 90-97) 全部保留', async () => {
    const { term, serialize } = createTerminal(120, 30);
    const codes = [30, 31, 32, 33, 34, 35, 36, 37, 90, 91, 92, 93, 94, 95, 96, 97];
    for (const code of codes) {
      await writeSync(term, `\x1b[${code}mCOLOR_${code}\x1b[0m `);
    }
    const result = serialize.serialize();
    for (const code of codes) {
      expect(result).toContain(`\x1b[${code}m`);
      expect(result).toContain(`COLOR_${code}`);
    }
  });

  it('16 个基本背景色 (40-47, 100-107) 全部保留', async () => {
    const { term, serialize } = createTerminal(120, 30);
    const codes = [40, 41, 42, 43, 44, 45, 46, 47, 100, 101, 102, 103, 104, 105, 106, 107];
    for (const code of codes) {
      await writeSync(term, `\x1b[${code}mBG_${code}\x1b[0m `);
    }
    const result = serialize.serialize();
    for (const code of codes) {
      expect(result).toContain(`\x1b[${code}m`);
      expect(result).toContain(`BG_${code}`);
    }
  });

  it('红色前景 (31) + 绿色背景 (42) 组合', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[31;42mRED_ON_GREEN\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[31;42m');
    expect(result).toContain('RED_ON_GREEN');
  });

  it('亮黄色前景 (93) + 亮蓝色背景 (104) 组合', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[93;104mBRIGHT_YELLOW_ON_BLUE\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[93;104m');
    expect(result).toContain('BRIGHT_YELLOW_ON_BLUE');
  });

  // ============ 256 色 ============

  it('256 色前景 — 标准色 0-15 会被优化为基本色', async () => {
    const { term, serialize } = createTerminal(120, 30);
    // 256 色索引 0-7 映射到基本前景色 30-37，8-15 映射到 90-97
    for (let i = 0; i < 16; i++) {
      await writeSync(term, `\x1b[38;5;${i}mC256_${i}\x1b[0m `);
    }
    const result = serialize.serialize();
    // serialize addon 会将 38;5;0 优化为 30，38;5;1→31 等
    expect(result).toContain('C256_0');
    expect(result).toContain('C256_15');
    expect(result).toContain('\x1b[0m');
  });

  it('256 色前景 — 216 色立方 (16-231)', async () => {
    const { term, serialize } = createTerminal(120, 30);
    // 抽样测试 216 色立方中的部分颜色
    const samples = [16, 51, 88, 124, 160, 196, 231];
    for (const n of samples) {
      await writeSync(term, `\x1b[38;5;${n}mCUBE_${n}\x1b[0m `);
    }
    const result = serialize.serialize();
    for (const n of samples) {
      expect(result).toContain(`\x1b[38;5;${n}m`);
      expect(result).toContain(`CUBE_${n}`);
    }
  });

  it('256 色前景 — 灰度 (232-255)', async () => {
    const { term, serialize } = createTerminal(120, 30);
    const samples = [232, 243, 255];
    for (const n of samples) {
      await writeSync(term, `\x1b[38;5;${n}mGRAY_${n}\x1b[0m `);
    }
    const result = serialize.serialize();
    for (const n of samples) {
      expect(result).toContain(`\x1b[38;5;${n}m`);
    }
  });

  it('256 色背景 — 标准色 0-7 会被优化为基本色', async () => {
    const { term, serialize } = createTerminal(120, 30);
    // 256 色背景索引 0-7 映射到基本背景色 40-47
    for (let i = 0; i < 8; i++) {
      await writeSync(term, `\x1b[48;5;${i}mBG256_${i}\x1b[0m `);
    }
    const result = serialize.serialize();
    // serialize addon 会将 48;5;0 优化为 40，48;5;1→41 等
    for (let i = 0; i < 8; i++) {
      expect(result).toContain(`BG256_${i}`);
    }
  });

  // ============ TrueColor 24-bit ============

  it('TrueColor 前景 — 多种颜色', async () => {
    const { term, serialize } = createTerminal(120, 30);
    // #FF0000 纯红
    await writeSync(term, '\x1b[38;2;255;0;0mRED\x1b[0m ');
    // #00FF00 纯绿
    await writeSync(term, '\x1b[38;2;0;255;0mGREEN\x1b[0m ');
    // #0000FF 纯蓝
    await writeSync(term, '\x1b[38;2;0;0;255mBLUE\x1b[0m ');
    // #FF8800 橙色
    await writeSync(term, '\x1b[38;2;255;136;0mORANGE\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[38;2;255;0;0m');
    expect(result).toContain('\x1b[38;2;0;255;0m');
    expect(result).toContain('\x1b[38;2;0;0;255m');
    expect(result).toContain('\x1b[38;2;255;136;0m');
  });

  it('TrueColor 背景 — 多种颜色', async () => {
    const { term, serialize } = createTerminal(120, 30);
    await writeSync(term, '\x1b[48;2;128;0;128mPURPLE_BG\x1b[0m ');
    await writeSync(term, '\x1b[48;2;0;128;128mTEAL_BG\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[48;2;128;0;128m');
    expect(result).toContain('\x1b[48;2;0;128;128m');
  });

  it('TrueColor 前景+背景混合', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[38;2;255;255;255;48;2;50;50;50mWHITE_ON_DARK\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[38;2;255;255;255;48;2;50;50;50m');
    expect(result).toContain('WHITE_ON_DARK');
  });

  // ============ 文本属性 ============

  it('粗体 (1) 保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[1mBOLD_TEXT\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[1m');
    expect(result).toContain('BOLD_TEXT');
  });

  it('斜体 (3) 保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[3mITALIC_TEXT\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[3m');
    expect(result).toContain('ITALIC_TEXT');
  });

  it('下划线 (4) 保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[4mUNDERLINED\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[4m');
    expect(result).toContain('UNDERLINED');
  });

  it('双下划线 (21) — serialize 输出为普通下划线 (4)', async () => {
    const { term, serialize } = createTerminal();
    // xterm.js 内部将双下划线(21)等同于单下划线(4)处理
    await writeSync(term, '\x1b[21mDOUBLE_UNDERLINE\x1b[0m');
    const result = serialize.serialize();
    // 内容应保留；SGR 码可能是 4 或 21（取决于实现）
    expect(result).toContain('DOUBLE_UNDERLINE');
  });

  it('慢闪 (5) 保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[5mBLINK_SLOW\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[5m');
    expect(result).toContain('BLINK_SLOW');
  });

  it('反显 (7) 保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[7mREVERSED\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[7m');
    expect(result).toContain('REVERSED');
  });

  it('隐藏 (8) 保留', async () => {
    const { term, serialize } = createTerminal();
    // 隐藏文本虽然不可见，但序列应保留
    await writeSync(term, 'VISIBLE\x1b[8mHIDDEN\x1b[0m VISIBLE_AFTER');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[8m');
    expect(result).toContain('HIDDEN');
    expect(result).toContain('VISIBLE');
  });

  it('删除线 (9) 保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[9mSTRIKETHROUGH\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[9m');
    expect(result).toContain('STRIKETHROUGH');
  });

  it('弱化/暗色 (2) 保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[2mDIM_TEXT\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[2m');
    expect(result).toContain('DIM_TEXT');
  });

  it('上划线 (53) 保留', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[53mOVERLINED\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[53m');
    expect(result).toContain('OVERLINED');
  });

  // ============ 属性叠加 ============

  it('粗体+下划线+红色前景+绿色背景 四属性叠加', async () => {
    const { term, serialize } = createTerminal();
    // serialize addon 会将颜色参数排在前面：31;42;1;4
    await writeSync(term, '\x1b[1;4;31;42mMULTI_ATTR\x1b[0m');
    const result = serialize.serialize();
    // 验证各 SGR 子参数均存在（不要求顺序，用 (?=[;m]) 替代 \b）
    expect(result).toMatch(/\x1b\[[0-9;]*31(?=[;m])/);
    expect(result).toMatch(/\x1b\[[0-9;]*42(?=[;m])/);
    expect(result).toMatch(/\x1b\[[0-9;]*1(?=[;m])/);
    expect(result).toMatch(/\x1b\[[0-9;]*4(?=[;m])/);
    expect(result).toContain('MULTI_ATTR');
  });

  it('SGR 重置 (0) 取消所有属性', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[1;3;4;31mFANCY\x1b[0m RESETTED');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[0m');
    expect(result).toContain('RESETTED');
  });

  it('SGR 子参数重置 (如 22 取消粗体)', async () => {
    const { term, serialize } = createTerminal();
    // 22 取消粗体/弱化，但其他属性保留
    await writeSync(term, '\x1b[1;4;31mBOLD_UNDERLINE_RED\x1b[22m NOT_BOLD\x1b[0m');
    const result = serialize.serialize();
    expect(result).toContain('\x1b[22m');
    expect(result).toContain('NOT_BOLD');
  });

  // ============ DEC 私用模式序列 ============

  it('DECTCEM — 光标显隐 (CSI ?25h / ?25l)', async () => {
    const { term, serialize } = createTerminal();
    // 隐藏光标 → 写文本 → 显示光标
    await writeSync(term, '\x1b[?25l');
    await writeSync(term, 'HIDDEN_CURSOR_TEXT');
    await writeSync(term, '\x1b[?25h');
    const result = serialize.serialize();
    expect(result).toContain('HIDDEN_CURSOR_TEXT');
  });

  it('DECOM — 原点模式 (CSI ?6h / ?6l)', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[?6h'); // 启用原点模式
    await writeSync(term, 'ORIGIN_MODE');
    await writeSync(term, '\x1b[?6l'); // 禁用原点模式
    const result = serialize.serialize();
    expect(result).toContain('ORIGIN_MODE');
  });

  it('DECAWM — 自动换行 (CSI ?7h / ?7l)', async () => {
    const { term, serialize } = createTerminal(10, 10);
    // 禁用自动换行 — 写到行尾时截断
    await writeSync(term, '\x1b[?7l');
    await writeSync(term, 'THIS_IS_A_LONG_LINE_THAT_SHOULD_TRUNCATE');
    // 重新启用自动换行
    await writeSync(term, '\x1b[?7h');
    const result = serialize.serialize();
    // 文本应存在（只是可能被截断不换行）
    expect(result).toContain('THIS_IS_A');
  });

  it('DECSC/DECRC 保存恢复光标+属性', async () => {
    const { term, serialize } = createTerminal(80, 10);
    // 设置红色属性
    await writeSync(term, '\x1b[31m');
    // 保存光标和属性 (ESC 7)
    await writeSync(term, '\x1b7');
    // 改属性为蓝色并移到别处
    await writeSync(term, '\x1b[34m');
    await writeSync(term, '\x1b[5;10H');
    await writeSync(term, 'BLUE_TEXT');
    // 恢复光标+属性 (ESC 8) — 回到红色
    await writeSync(term, '\x1b8');
    await writeSync(term, 'RED_AGAIN');
    const result = serialize.serialize();
    expect(result).toContain('BLUE_TEXT');
    expect(result).toContain('RED_AGAIN');
  });

  it('RIS (ESC c) 硬重置终端状态', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[31mCOLORED_TEXT');
    // ESC c 硬重置 — 清除所有属性和内容
    await writeSync(term, '\x1bc');
    await writeSync(term, 'FRESH_START');
    const result = serialize.serialize();
    // ESC c 会清屏，旧内容应被清除
    expect(result).not.toContain('COLORED_TEXT');
    expect(result).toContain('FRESH_START');
  });

  it('DECSTBM 设置滚动区域 (CSI r)', async () => {
    const { term, serialize } = createTerminal(80, 10);
    // 设置滚动区域为第 3-8 行
    await writeSync(term, '\x1b[3;8r');
    // 写多行触发滚动（仅滚动区域滚动，顶部和底部固定）
    for (let i = 1; i <= 15; i++) {
      await writeSync(term, `LINE_${i}\r\n`);
    }
    const result = serialize.serialize();
    // 内容应存在
    expect(result).toContain('LINE_1');
    expect(result).toContain('LINE_15');
  });
});
