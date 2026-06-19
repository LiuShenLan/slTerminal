import { expect, browser } from '@wdio/globals';

describe('slTerminal E2E', () => {
  it('应正常启动 Tauri 应用，标题为 slTerminal', async () => {
    // 等 webview 加载完成、document.title 就绪（避免取到瞬态值 localhost/空）
    await browser.waitUntil(
      async () => (await browser.getTitle()) === 'slTerminal',
      { timeout: 10000, timeoutMsg: '标题未就绪' },
    );
    const title = await browser.getTitle();
    expect(title).toBe('slTerminal');
  });
});
