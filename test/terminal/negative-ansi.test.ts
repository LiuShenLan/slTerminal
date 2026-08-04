// L3 终端渲染测试 — 反向/异常 ANSI 输入负面用例（E2E-14）
// 使用 @xterm/headless：非法 ANSI、截断多字节序列、嵌套/未终止 OSC、异常 resize、
// 非法 SGR 参数、超长 CSI 参数——headless 不崩溃且网格状态可恢复。
// 一手证据：xterm 解析器对非法/未完成序列按 VT500 规范静默忽略（InputHandler），
// resize 经 _verifyIntegers 整数守卫 + MINIMUM_COLS/MINIMUM_ROWS clamp
// （node_modules/@xterm/xterm/src/common/CoreTerminal.ts:187-193）。

import { describe, it, expect } from 'vitest';
import { Terminal } from '@xterm/headless';

/** 等待 write 完成的辅助函数 */
function writeSync(term: Terminal, data: string): Promise<void> {
  return new Promise<void>((resolve) => {
    term.write(data, resolve);
  });
}

function createTerminal(cols = 40, rows = 5): Terminal {
  return new Terminal({ cols, rows, allowProposedApi: true });
}

describe('L3 终端渲染 — 反向/异常 ANSI（E2E-14）', () => {
  it('孤立 ESC / 悬空 CSI 前缀不崩溃，后续完整文本可恢复', async () => {
    const term = createTerminal();
    // 孤立 ESC 会消费下一字符（'S' 被吞），悬空 CSI 前缀无最终字节被忽略
    await writeSync(term, '\x1b\x1b[');
    await writeSync(term, 'SAFE');
    expect(term.buffer.active.getLine(0)!.translateToString().trim()).toBe('AFE');
    // 状态可恢复：完整 ANSI 序列后文本正常渲染
    await writeSync(term, '\x1b[32mGREEN\x1b[0m');
    expect(term.buffer.active.getLine(0)!.translateToString()).toContain('GREEN');
  });

  it('截断 CSI 序列不崩溃，被吞文本后的完整 write 正常', async () => {
    const term = createTerminal();
    // '\x1b[31' 缺最终字节——'X' 被当作 CSI 最终字节消费，不渲染
    await writeSync(term, '\x1b[31');
    await writeSync(term, 'X');
    expect(term.buffer.active.getLine(0)!.translateToString().trim()).toBe('');
    // 后续完整序列正常：'\x1b[0m' 复位 + 'Y' 渲染（状态可恢复）
    await writeSync(term, '\x1b[0mY');
    expect(term.buffer.active.getLine(0)!.translateToString().trim()).toBe('Y');
  });

  it('截断多字节序列（孤立 surrogate）不崩溃且保留', async () => {
    const term = createTerminal();
    // 孤立高位 surrogate（Emoji 拆半）——xterm 按 code unit 原样落格，不崩溃
    await writeSync(term, '\ud83d');
    await writeSync(term, 'OK');
    expect(term.buffer.active.getLine(0)!.translateToString().trim()).toBe('\ud83dOK');
  });

  it('未终止 OSC 不崩溃，BEL 终止后标题累积完整 + 后续文本正常', async () => {
    const term = createTerminal();
    const titles: string[] = [];
    term.onTitleChange((title) => titles.push(title));
    // OSC 未终止（无 BEL/ST）——期间文本被 OSC 字符串吞掉，标题不触发
    await writeSync(term, '\x1b]2;UNTERMINATED');
    await writeSync(term, 'TAIL');
    expect(titles).toEqual([]);
    // BEL 终止：OSC 内容累积（UNTERMINATED + TAIL）一次性触发
    await writeSync(term, '\x07');
    expect(titles).toEqual(['UNTERMINATEDTAIL']);
    // 状态可恢复：终止后文本正常渲染
    await writeSync(term, 'DONE');
    expect(term.buffer.active.getLine(0)!.translateToString().trim()).toBe('DONE');
  });

  it('嵌套 OSC（OSC 内嵌 CSI 序列）不崩溃，后续文本正常', async () => {
    const term = createTerminal();
    // OSC 8 字符串内嵌 CSI 序列（ESC 在 OSC 内属数据，仅 BEL/ST 终止）
    await writeSync(term, '\x1b]8;;https://x.example\x1b[31m\x1b\\');
    await writeSync(term, 'AFTER');
    expect(term.buffer.active.getLine(0)!.translateToString().trim()).toBe('AFTER');
  });

  it('异常 resize(0,0) / resize(-5,-5) 被 clamp 到最小网格且不崩溃', async () => {
    // CoreTerminal.resize：NaN 忽略，其余经 Math.max clamp 到
    // MINIMUM_COLS=2 / MINIMUM_ROWS=1（CoreTerminal.ts:187-193）
    const term = createTerminal(80, 24);
    term.resize(0, 0);
    expect(term.cols).toBe(2);
    expect(term.rows).toBe(1);
    term.resize(-5, -5);
    expect(term.cols).toBe(2);
    expect(term.rows).toBe(1);
    // clamp 后网格可继续使用
    await writeSync(term, 'X');
    expect(term.buffer.active.getLine(0)!.translateToString().trim()).toBe('X');
  });

  it('异常 resize(NaN) 抛整数守卫错误（headless 契约），非静默崩溃', async () => {
    // headless 6.0 resize 经 _verifyIntegers 校验：NaN/Infinity/非整数抛
    // "This API only accepts integers"（生产 usePtyResize 有 NaN guard，L2 已覆盖；
    // 此处锁定 xterm 契约本身）
    const term = createTerminal(80, 24);
    expect(() => term.resize(NaN, NaN)).toThrow('This API only accepts integers');
    expect(() => term.resize(80.5, 24)).toThrow('This API only accepts integers');
    // 抛错后终端仍可用
    await writeSync(term, 'STILL_WORKS');
    expect(term.buffer.active.getLine(0)!.translateToString()).toContain('STILL_WORKS');
  });

  it('非法 SGR 参数不崩溃：超范围码忽略、空参数跳过、缺省参数按规范解析', async () => {
    const term = createTerminal();
    // 999 超出 SGR 范围 → 忽略（X 保持默认色）；';m' 空参数 → 跳过；
    // '1;;3' → 解析为 [1,3]（空参数被跳过），3=斜体生效
    await writeSync(term, '\x1b[999mX\x1b[;mY\x1b[1;;3mZ');
    const line = term.buffer.active.getLine(0)!;
    const c0 = line.getCell(0)!;
    const c1 = line.getCell(1)!;
    const c2 = line.getCell(2)!;
    expect(line.translateToString().trim()).toBe('XYZ');
    expect(c0.getFgColorMode()).toBe(0); // CM_DEFAULT（999 忽略）
    expect(c0.getFgColor()).toBe(-1);
    expect(c1.getFgColorMode()).toBe(0); // 空参数序列被跳过
    expect(c2.isItalic()).toBeTruthy(); // 1;;3 → 粗体+斜体（空参数跳过）
  });

  it('超长 CSI 参数（500 个参数）不崩溃，文本正常渲染', async () => {
    const term = createTerminal();
    // 500 个 '1;' 参数 + 'm'——参数表内部容量处理，不崩溃不吞文本
    await writeSync(term, '\x1b[' + '1;'.repeat(500) + 'mTEXT');
    expect(term.buffer.active.getLine(0)!.translateToString().trim()).toBe('TEXT');
  });
});
