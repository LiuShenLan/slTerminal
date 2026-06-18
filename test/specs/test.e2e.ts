import { expect, browser } from '@wdio/globals';

describe('slTerminal E2E', () => {
  it('应正常启动 Tauri 应用', async () => {
    // 验证 session 创建成功
    const title = await browser.getTitle();
    expect(title).toBeTruthy();
  });
});
