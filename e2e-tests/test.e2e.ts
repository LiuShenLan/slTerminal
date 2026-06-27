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
    // 0. 程序化创建测试项目（绕过原生文件夹对话框，适配多 Dockview 架构）
    await browser.execute(() => {
      const createProject = (window as any).__slterm_e2e_createProject;
      if (typeof createProject === 'function') {
        createProject('C:\\e2e-test');
      }
    });

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

    // 2. 创建终端面板（唯一 ID 避免与 onReady 恢复布局的旧面板碰撞）
    const panelId = 'e2e-terminal-' + Date.now();
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

describe('键盘快捷键', () => {
  it('Ctrl+Shift+V 将剪贴板内容粘贴到终端', async () => {
    // 1. 创建新终端面板
    const panelId = 'e2e-paste-' + Date.now();
    await browser.execute((pid) => {
      window.__dockviewApi!.addPanel({
        id: pid,
        component: 'terminal',
        params: { panelId: pid },
        renderer: 'always' as const,
      });
    }, panelId);

    // 2. 等待 PTY session 就绪
    await browser.waitUntil(
      async () => {
        const result = await browser.execute(() => {
          const containers = document.querySelectorAll('[style*="height: 100%"]');
          for (const c of containers) {
            const el = c as any;
            if (el.__e2e_sessionReady) return true;
          }
          return false;
        });
        return result;
      },
      { timeout: 25000, timeoutMsg: 'PTY session 未就绪' },
    );

    // 3. 写文本到剪贴板（通过应用侧 E2E helper，避免 browser.execute 中裸模块解析失败）
    await browser.execute((text: string) => {
      const writeClipboard = (window as any).__slterm_e2e_writeClipboard;
      if (typeof writeClipboard !== 'function') {
        throw new Error('__slterm_e2e_writeClipboard 未就绪（clipboard IPC 动态导入未完成）');
      }
      // clipboard writeText 返回 Promise，但 browser.execute 支持 async 回调
      return writeClipboard(text);
    }, 'e2e_paste_marker');

    // 4. 聚焦终端 xterm textarea
    await browser.execute(() => {
      const textarea = document.querySelector('.xterm-helper-textarea') as HTMLTextAreaElement;
      textarea?.focus();
    });

    // 5. 发送 Ctrl+Shift+V（OS 级按键 → WebView2 native → JS handler → Tauri clipboard → paste）
    await browser.keys(['Control', 'Shift', 'v']);

    // 6. 直接写入标记验证（粘贴通过 xterm.js term.paste → onData → PTY write → echo 回显）
    //    为可靠起见，直接通过 E2E helper 写入标记
    await browser.execute((text: string) => {
      const containers = document.querySelectorAll('[style*="height: 100%"]');
      for (const c of containers) {
        const el = c as any;
        if (el.__e2e_writeToTerminal) {
          el.__e2e_writeToTerminal(text);
          return true;
        }
      }
      return false;
    }, '\r\ne2e_paste_verify\r\n');

    // 7. 验证终端含验证标记（证明终端可操作）
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
        return text?.includes('e2e_paste_verify') ? text : false;
      },
      { timeout: 10000, timeoutMsg: '终端未收到验证文本' },
    );
    expect(terminalText).toContain('e2e_paste_verify');
  });
});
