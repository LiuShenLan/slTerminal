import { expect, browser, $ } from '@wdio/globals';

describe('slTerminal E2E', () => {
  it('应正常启动并显示暗色窗口', async () => {
    // 等待 WebView2 加载完毕
    await browser.pause(2000);

    // 检查 body 背景为暗色
    const body = await $('body');
    const bgColor = await body.getCSSProperty('background-color');
    const rgba = bgColor.value as string;
    // rgba(r, g, b, a) 格式解析
    const match = rgba.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
    expect(match).not.toBeNull();
    if (match) {
      const r = parseInt(match[1]);
      const g = parseInt(match[2]);
      const b = parseInt(match[3]);
      const luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
      expect(luma).toBeLessThan(100);
    }
  });

  it('应包含 Dockview 容器', async () => {
    const dockview = await $('.dockview-theme-dark');
    expect(dockview).toBeExisting();
  });
});
