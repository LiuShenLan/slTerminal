import { describe, it, expect } from 'vitest';
import { Terminal } from '@xterm/headless';
import { SerializeAddon } from '@xterm/addon-serialize';

describe('L3 终端渲染', () => {
  it('feed "hi" 后 serialize 结果应包含 "hi"', async () => {
    const term = new Terminal({
      cols: 80,
      rows: 24,
      allowProposedApi: true,
    });
    const serializeAddon = new SerializeAddon();
    term.loadAddon(serializeAddon);

    // write 是异步的，必须通过 callback 等待解析完成再 serialize
    await new Promise<void>((resolve) => {
      term.write('hi', resolve);
    });

    const result = serializeAddon.serialize();
    expect(result).toContain('hi');
  });
});
