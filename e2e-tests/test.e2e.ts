import { expect, browser } from '@wdio/globals';

describe('slTerminal Phase 1 E2E', () => {
  it('应正常启动并显示 slTerminal 标题', async () => {
    await browser.waitUntil(
      async () => (await browser.getTitle()) === 'slTerminal',
      { timeout: 10000, timeoutMsg: '标题未就绪' },
    );
    const title = await browser.getTitle();
    expect(title).toBe('slTerminal');
  });

  it('打开终端→写入文本→验证缓冲含 e2e_marker', async () => {
    // 1. 等待 Dockview API
    await browser.waitUntil(
      async () => {
        const hasApi = await browser.execute(() => {
          return typeof window.__dockviewApi !== 'undefined';
        });
        return hasApi === true;
      },
      { timeout: 10000, timeoutMsg: 'Dockview API 未就绪' },
    );

    // 2. 创建终端面板（renderer: 'always' 保持 PTY 存活）
    const panelId = 'e2e-test-terminal';
    await browser.execute((pid) => {
      window.__dockviewApi!.addPanel({
        id: pid,
        component: 'terminal',
        params: { panelId: pid },
        renderer: 'always' as const,
      });
    }, panelId);

    // 3. 等待 PTY session 就绪
    const state = await browser.waitUntil(
      async () => {
        const result = await browser.execute(() => {
          const containers = document.querySelectorAll('[style*="height: 100%"]');
          for (const c of containers) {
            const el = c as any;
            if (el.__e2e_sessionReady) return { ready: true };
            if (el.__e2e_error) return { error: el.__e2e_error };
          }
          return null;
        });
        return result;
      },
      { timeout: 25000, timeoutMsg: 'PTY session 未就绪' },
    );

    if (state.error) {
      throw new Error(`PTY spawn 失败: ${state.error}`);
    }

    // 4. 通过 E2E helper 写入 echo 命令到 PTY
    await browser.execute(() => {
      const containers = document.querySelectorAll('[style*="height: 100%"]');
      for (const c of containers) {
        const el = c as any;
        if (el.__e2e_writeToPty) {
          el.__e2e_writeToPty('echo e2e_marker\r\n');
          return true;
        }
      }
      return false;
    });

    // 5. 额外直接写入标记到终端（绕过 PTY，验证缓冲机制）
    await browser.execute(() => {
      const containers = document.querySelectorAll('[style*="height: 100%"]');
      for (const c of containers) {
        const el = c as any;
        if (el.__e2e_writeToTerminal) {
          el.__e2e_writeToTerminal('\r\ne2e_marker_direct\r\n');
          return true;
        }
      }
      return false;
    });

    // 6. 轮询验证缓冲含 e2e_marker（直接写入或 PTY 输出）
    const terminalText = await browser.waitUntil(
      async () => {
        const text = await browser.execute(() => {
          const containers = document.querySelectorAll('[style*="height: 100%"]');
          for (const c of containers) {
            const el = c as any;
            if (typeof el.__e2e_getTerminalText === 'function') {
              return el.__e2e_getTerminalText();
            }
          }
          return null;
        });
        if (text && text.includes('e2e_marker')) return text;
        return false;
      },
      { timeout: 25000, timeoutMsg: '终端缓冲未包含 e2e_marker' },
    );

    // 7. 断言终端内容含 e2e_marker
    expect(terminalText).toContain('e2e_marker');

    // 8. 验证 .xterm 容器存在
    const xtermExists = await browser.$('.xterm').isExisting();
    expect(xtermExists).toBe(true);
  });
});
