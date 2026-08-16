// L3 终端渲染测试 — 生产 theme.ts 终端选项（E2E-02）
// 使用 @xterm/headless + 生产 src/panels/terminal/theme.ts 的 terminalOptions 创建 Terminal
// 覆盖：16 色 ANSI 与主题色板一致、CSI>1u 可激活 Kitty、scrollback 容量生效、
//       drawBoldTextInBrightColors 亮色映射（配置层 + cell 属性）
//
// 定位声明（DOC-02/E2E-04）：L3 = 网格状态正确性，非渲染正确性。本文件验证生产选项在
// headless 网格上的可观察语义；Kitty 编码行为与亮色渲染映射依赖 DOM/渲染器层，
// headless 不可触发，由 L4 真实 WebView2 视觉回归验收。

import { describe, it, expect } from 'vitest';
import { Terminal } from '@xterm/headless';
import { terminalOptions } from '../../src/panels/terminal/theme';

/** 等待 write 完成的辅助函数 */
function writeSync(term: Terminal, data: string): Promise<void> {
  return new Promise<void>((resolve) => {
    term.write(data, resolve);
  });
}

// xterm 内部 ColorMode 常量（node_modules/@xterm/xterm/src/common/buffer/Constants.ts:100-104，
// headless 6.0 同源）：getFgColorMode()/getBgColorMode() 返回值
const CM_P16 = 0x1000000; // 16 色 palette 模式

// ANSI 16 色编码：前景 30-37/90-97，背景 40-47/100-107（xterm palette 索引 0-15 约定）
const FG_CODES = [30, 31, 32, 33, 34, 35, 36, 37, 90, 91, 92, 93, 94, 95, 96, 97];
const BG_CODES = [40, 41, 42, 43, 44, 45, 46, 47, 100, 101, 102, 103, 104, 105, 106, 107];

describe('L3 终端渲染 — 生产 terminalOptions（E2E-02）', () => {
  it('16 色 ANSI 前景与主题色板一致（palette 索引 + 色值快照）', async () => {
    const term = new Terminal({ ...terminalOptions, rows: 5, cols: 80 });
    // 每色一个单字符（A..P），列索引即 palette 索引
    for (let i = 0; i < 16; i++) {
      await writeSync(term, `\x1b[${FG_CODES[i]}m${String.fromCharCode(65 + i)}\x1b[0m`);
    }
    const line = term.buffer.active.getLine(0)!;
    for (let i = 0; i < 16; i++) {
      const cell = line.getCell(i)!;
      // ANSI 码 → xterm palette 索引 0-15（30-37→0-7，90-97→8-15）
      expect(cell.getFgColorMode()).toBe(CM_P16);
      expect(cell.getFgColor()).toBe(i);
      expect(cell.getCode()).toBe(String.fromCharCode(65 + i).charCodeAt(0));
    }
    // 主题色板快照（生产 theme.ts 现值 = linear 方案 terminal 段，附录 A 契约；
    // 索引约定：0-7=black..white，8-15=brightBlack..brightWhite）
    expect(terminalOptions.theme).toMatchObject({
      black: '#0a0a0b',
      red: '#d9706b',
      green: '#93b573',
      yellow: '#d6b25e',
      blue: '#7fa8e8',
      magenta: '#b48ce0',
      cyan: '#6fbfc4',
      white: '#cfcac1',
      brightBlack: '#7d7871',
      brightRed: '#e2877f',
      brightGreen: '#a8c98d',
      brightYellow: '#e3c67f',
      brightBlue: '#9dbfee',
      brightMagenta: '#c6a6e8',
      brightCyan: '#8dd0d4',
      brightWhite: '#f0ede8',
    });
  });

  it('16 色 ANSI 背景与主题色板一致（palette 索引）', async () => {
    const term = new Terminal({ ...terminalOptions, rows: 5, cols: 80 });
    for (let i = 0; i < 16; i++) {
      await writeSync(term, `\x1b[${BG_CODES[i]}m${String.fromCharCode(65 + i)}\x1b[0m`);
    }
    const line = term.buffer.active.getLine(0)!;
    for (let i = 0; i < 16; i++) {
      const cell = line.getCell(i)!;
      // ANSI 背景码 → palette 索引 0-15（40-47→0-7，100-107→8-15）
      expect(cell.getBgColorMode()).toBe(CM_P16);
      expect(cell.getBgColor()).toBe(i);
    }
  });

  it('CSI>1u 可激活 Kitty — 生产 vtExtensions 配置 + 激活序列解析安全', async () => {
    // 生产配置单点：vtExtensions.kittyKeyboard = true（theme.ts:43；L2 STS-05 互补锁配置）
    expect(terminalOptions.vtExtensions).toEqual({ kittyKeyboard: true });
    const term = new Terminal({ ...terminalOptions, rows: 5, cols: 40 });
    // headless 6.0 将 vtExtensions 移出公开 options（仅核心服务消费），term.options 读不回，
    // 配置有效性以生产对象断言为准
    await writeSync(term, '\x1b[>1u'); // push Disambiguate 模式 flag（claude 激活路径）
    await writeSync(term, 'AFTER_KITTY_PUSH');
    // 激活序列解析安全：不抛异常、后续输入正常渲染
    expect(term.buffer.active.getLine(0)!.translateToString()).toContain('AFTER_KITTY_PUSH');
    // 定位声明：Kitty 编码行为（Ctrl+Enter → CSI u）依赖 DOM KeyboardService，
    // headless 无键盘服务不可触发（E2E-04/DOC-02），由 L4 真实 WebView2 验收
  });

  it('scrollback 容量生效 — 生产配置 5000，超出行被淘汰', async () => {
    // 生产配置：scrollback = 5000（theme.ts:38）
    expect(terminalOptions.scrollback).toBe(5000);
    const term = new Terminal({ ...terminalOptions, rows: 24 });
    expect(term.options.scrollback).toBe(5000); // headless 读回
    // 写 5050 行（总容量 = scrollback 5000 + viewport 24 = 5024，淘汰最老 26 行）
    const lines: string[] = [];
    for (let i = 0; i < 5050; i++) {
      lines.push(`SL${String(i).padStart(4, '0')}`);
    }
    await writeSync(term, lines.join('\r\n'));
    const buffer = term.buffer.active;
    // 容量精确 = 5000 + 24（scrollback 不无限增长）
    expect(buffer.length).toBe(5024);
    // 首行为淘汰后保留的最老行：SL0026（SL0000..SL0025 共 26 行被淘汰）
    expect(buffer.getLine(0)!.translateToString().trim()).toBe('SL0026');
    // 末行为最新写入行 SL5049
    expect(buffer.getLine(5023)!.translateToString().trim()).toBe('SL5049');
  });

  it('drawBoldTextInBrightColors=true — 配置读回 + 粗体 cell 属性（渲染层映射由 L4）', async () => {
    // 生产配置：drawBoldTextInBrightColors = true（theme.ts:40，显式声明消除隐式默认依赖）
    expect(terminalOptions.drawBoldTextInBrightColors).toBe(true);
    const term = new Terminal({ ...terminalOptions, rows: 5, cols: 40 });
    expect(term.options.drawBoldTextInBrightColors).toBe(true); // headless 读回
    await writeSync(term, '\x1b[1;31mX\x1b[0m');
    const cell = term.buffer.active.getLine(0)!.getCell(0)!;
    // 粗体标志落格（isBold 返回 FLAG_BOLD 位值，toBeTruthy 即可）
    expect(cell.isBold()).toBeTruthy();
    // 亮色映射（palette 0-7 粗体 → 8-15 亮色）发生在渲染器层：buffer cell 保持原 palette
    // 索引 1（红色），headless 无渲染器不可观察映射结果（E2E-04/DOC-02 定位声明），
    // 由 L4 真实 WebView2 视觉回归验收
    expect(cell.getFgColorMode()).toBe(CM_P16);
    expect(cell.getFgColor()).toBe(1);
  });
});
