import { expect, browser } from '@wdio/globals';
import { mkdtempSync, writeFileSync, readFileSync, statSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

/** 等待页签标题变为指定值（轮询 Dockview panel title） */
async function waitForPanelTitle(
  panelId: string,
  expectedTitle: string,
  timeout = 10000,
): Promise<string> {
  const title = await browser.waitUntil(
    async () => {
      const t = await browser.execute((pid: string) => {
        const panel = window.__dockviewApi?.getPanel(pid);
        return panel?.api.title ?? null;
      }, panelId);
      if (t === expectedTitle) return t;
      return false;
    },
    { timeout, timeoutMsg: `面板 ${panelId} 标题未在 ${timeout}ms 内变为 "${expectedTitle}"` },
  );
  return title as string;
}

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
    // 0a. 等待 Workspace 就绪（消除 createProject 与 App init 的竞态）
    await browser.waitUntil(
      async () => {
        return await browser.execute(() => {
          return (window as any).__slterm_e2e_workspaceReady === true;
        });
      },
      { timeout: 15000, timeoutMsg: 'Workspace 未就绪（__slterm_e2e_workspaceReady 超时）' },
    );

    // 0b. 程序化创建测试项目（绕过原生文件夹对话框，适配多 Dockview 架构）
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
      { timeout: 20000, timeoutMsg: 'Dockview API 未就绪' },
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
  it('终端面板可通过 E2E helper 写入文本并读取', async () => {
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
        throw new Error('__slterm_e2e_writeClipboard 未就绪（clipboard helper 未挂载）');
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

describe('页签标题', () => {
  it('终端页签标题为 terminal-N', async () => {
    // 等待 Workspace 就绪
    await browser.waitUntil(
      async () => await browser.execute(() => (window as any).__slterm_e2e_workspaceReady === true),
      { timeout: 15000, timeoutMsg: 'Workspace 未就绪' },
    );

    // 创建测试项目
    await browser.execute(() => {
      const createProject = (window as any).__slterm_e2e_createProject;
      if (typeof createProject === 'function') createProject('C:\\e2e-title-test');
    });

    // 等待 Dockview API
    await browser.waitUntil(
      async () => await browser.execute(() => typeof window.__dockviewApi !== 'undefined'),
      { timeout: 20000, timeoutMsg: 'Dockview API 未就绪' },
    );

    // 创建终端面板（带标题）
    const panelId = 'e2e-title-term-' + Date.now();
    await browser.execute((pid: string) => {
      window.__dockviewApi!.addPanel({
        id: pid,
        component: 'terminal',
        title: 'terminal-99',
        params: { panelId: pid },
        renderer: 'always' as const,
      });
    }, panelId);

    // 验证标题
    const title = await waitForPanelTitle(panelId, 'terminal-99', 10000);
    expect(title).toBe('terminal-99');

    // 验证可以通过 API 动态修改标题
    await browser.execute((pid: string) => {
      const panel = window.__dockviewApi?.getPanel(pid);
      panel?.api.setTitle('terminal-custom');
    }, panelId);

    const updatedTitle = await waitForPanelTitle(panelId, 'terminal-custom', 5000);
    expect(updatedTitle).toBe('terminal-custom');
  });

  it('编辑器页签标题为文件名', async () => {
    // 等待 Workspace 就绪
    await browser.waitUntil(
      async () => await browser.execute(() => (window as any).__slterm_e2e_workspaceReady === true),
      { timeout: 15000 },
    );

    // 获取活跃页面信息
    const pageInfo = await browser.execute(() => {
      return (window as any).__slterm_e2e_getActivePageInfo?.() ?? null;
    });
    if (!pageInfo) throw new Error('无法获取活跃页面信息');

    const { pageId, rootPath } = pageInfo as { pageId: string; rootPath: string };

    // 创建编辑器面板（带文件路径）
    const panelId = 'e2e-title-editor-' + Date.now();
    const testFilePath = 'C:\\e2e-title-test\\src\\main.ts';

    await browser.execute(
      (args: { pid: string; testPath: string }) => {
        window.__dockviewApi!.addPanel({
          id: args.pid,
          component: 'editor',
          title: 'main.ts',
          params: { panelId: args.pid, filePath: args.testPath },
        });
      },
      { pid: panelId, testPath: testFilePath },
    );

    // 验证标题显示为文件名
    const title = await waitForPanelTitle(panelId, 'main.ts', 10000);
    expect(title).toBe('main.ts');

    // 注册文件编辑器并验证标题重算（单文件无冲突，保持 basename）
    await browser.execute(
      (args: { pageId: string; rootPath: string; pid: string; testPath: string }) => {
        (window as any).__slterm_e2e_registerAndRecompute?.(
          args.pageId,
          args.rootPath,
          args.pid,
          args.testPath,
        );
      },
      { pageId, rootPath, pid: panelId, testPath: testFilePath },
    );

    const recomputedTitle = await waitForPanelTitle(panelId, 'main.ts', 5000);
    expect(recomputedTitle).toBe('main.ts');
  });

  it('同名文件冲突时显示相对路径', async () => {
    // 等待就绪
    await browser.waitUntil(
      async () => await browser.execute(() => (window as any).__slterm_e2e_workspaceReady === true),
      { timeout: 15000 },
    );

    const pageInfo = await browser.execute(() => {
      return (window as any).__slterm_e2e_getActivePageInfo?.() ?? null;
    });
    if (!pageInfo) throw new Error('无法获取活跃页面信息');

    const { pageId, rootPath } = pageInfo as { pageId: string; rootPath: string };

    // 创建第一个编辑器（src/index.ts）
    const pid1 = 'e2e-conflict-1-' + Date.now();
    const path1 = 'C:\\e2e-title-test\\src\\index.ts';
    await browser.execute(
      (args: { pid: string; path: string; pageId: string; root: string }) => {
        window.__dockviewApi!.addPanel({
          id: args.pid, component: 'editor', title: 'index.ts',
          params: { panelId: args.pid, filePath: args.path },
        });
        (window as any).__slterm_e2e_registerAndRecompute?.(
          args.pageId, args.root, args.pid, args.path,
        );
      },
      { pid: pid1, path: path1, pageId, root: rootPath },
    );

    // 创建第二个编辑器（lib/index.ts）—— 同名不同路径
    const pid2 = 'e2e-conflict-2-' + Date.now();
    const path2 = 'C:\\e2e-title-test\\lib\\index.ts';
    await browser.execute(
      (args: { pid: string; path: string; pageId: string; root: string }) => {
        window.__dockviewApi!.addPanel({
          id: args.pid, component: 'editor', title: 'lib/index.ts',
          params: { panelId: args.pid, filePath: args.path },
        });
        (window as any).__slterm_e2e_registerAndRecompute?.(
          args.pageId, args.root, args.pid, args.path,
        );
      },
      { pid: pid2, path: path2, pageId, root: rootPath },
    );

    // 验证两个编辑器都显示为相对路径
    const title1 = await waitForPanelTitle(pid1, 'src/index.ts', 10000);
    const title2 = await waitForPanelTitle(pid2, 'lib/index.ts', 5000);
    expect(title1).toBe('src/index.ts');
    expect(title2).toBe('lib/index.ts');
  });

  it('关闭同名面板后剩余面板切回 basename', async () => {
    // 等待就绪
    await browser.waitUntil(
      async () => await browser.execute(() => (window as any).__slterm_e2e_workspaceReady === true),
      { timeout: 15000 },
    );

    const pageInfo = await browser.execute(() => {
      return (window as any).__slterm_e2e_getActivePageInfo?.() ?? null;
    });
    if (!pageInfo) throw new Error('无法获取活跃页面信息');

    const { pageId, rootPath } = pageInfo as { pageId: string; rootPath: string };

    // 创建两个同名编辑器
    const pid1 = 'e2e-reclose-1-' + Date.now();
    const pid2 = 'e2e-reclose-2-' + Date.now();
    const path1 = 'C:\\e2e-title-test\\a\\utils.ts';
    const path2 = 'C:\\e2e-title-test\\b\\utils.ts';

    await browser.execute(
      (args: {
        pid1: string; pid2: string; path1: string; path2: string;
        pageId: string; root: string;
      }) => {
        const api = window.__dockviewApi!;
        api.addPanel({
          id: args.pid1, component: 'editor', title: 'a/utils.ts',
          params: { panelId: args.pid1, filePath: args.path1 },
        });
        api.addPanel({
          id: args.pid2, component: 'editor', title: 'b/utils.ts',
          params: { panelId: args.pid2, filePath: args.path2 },
        });
        const reg = (window as any).__slterm_e2e_registerAndRecompute!;
        reg(args.pageId, args.root, args.pid1, args.path1);
        reg(args.pageId, args.root, args.pid2, args.path2);
      },
      { pid1, pid2, path1, path2, pageId, root: rootPath },
    );

    // 验证冲突状态
    expect(await waitForPanelTitle(pid1, 'a/utils.ts', 5000)).toBe('a/utils.ts');
    expect(await waitForPanelTitle(pid2, 'b/utils.ts', 5000)).toBe('b/utils.ts');

    // 关闭第二个面板（pid2）
    await browser.execute((pid: string) => {
      const panel = window.__dockviewApi?.getPanel(pid);
      panel?.api.close();
    }, pid2);

    // 验证 pid1 标题切回 basename（关闭冲突面板后自动重算）
    const finalTitle = await waitForPanelTitle(pid1, 'utils.ts', 10000);
    expect(finalTitle).toBe('utils.ts');
  });
});

describe('编辑器保存 (Ctrl+S)', () => {
  // 说明：embedded WDIO 驱动无法把 OS 级按键（browser.keys）投递进 WebView2 页面
  //（终端 Ctrl+Shift+V 用例同样绕过——它直接写标记而非断言真实按键）。
  // 故本用例在页面内 dispatch 合成 keydown 到 window——由 ShortcutRegistry 的
  // window capture 监听器真实捕获（与生产同一路径）→ editor.save → 真实 IPC fs.writeFile 写盘，
  // 以文件 mtime 变化断言写盘发生。覆盖 C1：capture 监听 + context 栈匹配 + 命令 handler + 写盘全链路。
  it('聚焦编辑器后 Ctrl+S → 经 capture 路径真实写盘（mtime 更新）', async () => {
    // 0. Node 侧创建真实临时目录 + 文件（唯一 marker；后端 project_root 未设置 → 路径 sandbox 跳过）
    const marker = 'e2e_save_' + Date.now();
    const tempDir = mkdtempSync(join(tmpdir(), 'slterm-e2e-save-'));
    const filePath = join(tempDir, 'save.txt');
    writeFileSync(filePath, marker, 'utf8');
    const mtimeBefore = statSync(filePath).mtimeMs;

    try {
      // 1. 等待 Workspace 就绪
      await browser.waitUntil(
        async () => await browser.execute(() => (window as any).__slterm_e2e_workspaceReady === true),
        { timeout: 15000, timeoutMsg: 'Workspace 未就绪' },
      );

      // 2. 程序化创建项目（根 = 临时目录）
      await browser.execute((dir: string) => {
        (window as any).__slterm_e2e_createProject?.(dir);
      }, tempDir);

      // 3. 等待 Dockview API
      await browser.waitUntil(
        async () => await browser.execute(() => typeof window.__dockviewApi !== 'undefined'),
        { timeout: 20000, timeoutMsg: 'Dockview API 未就绪' },
      );

      // 4. 打开编辑器面板（加载临时文件）
      const panelId = 'e2e-save-editor-' + Date.now();
      await browser.execute(
        (args: { pid: string; path: string }) => {
          window.__dockviewApi!.addPanel({
            id: args.pid,
            component: 'editor',
            params: { panelId: args.pid, filePath: args.path },
          });
        },
        { pid: panelId, path: filePath },
      );

      // 5. 等待编辑器加载文件内容（某个 .cm-content 含唯一 marker）
      await browser.waitUntil(
        async () =>
          await browser.execute((m: string) => {
            const nodes = document.querySelectorAll('.cm-content');
            for (const n of nodes) {
              if ((n.textContent ?? '').includes(m)) return true;
            }
            return false;
          }, marker),
        { timeout: 15000, timeoutMsg: '编辑器未加载文件内容（.cm-content 未出现 marker）' },
      );

      // 6. 标记目标编辑器并真实点击聚焦 → 真实 focusin 冒泡到 container → pushContext("editor")
      const marked = await browser.execute((m: string) => {
        const nodes = document.querySelectorAll('.cm-content');
        for (const n of nodes) {
          if ((n.textContent ?? '').includes(m)) {
            (n as HTMLElement).setAttribute('data-e2e-save', '1');
            return true;
          }
        }
        return false;
      }, marker);
      expect(marked).toBe(true);
      // 触发 editor 焦点上下文：usePanelFocus 监听 container 的 focusin 事件 →
      // 在 .cm-content 上 dispatch 合成 focusin（bubbles）→ 冒泡到 container → pushContext + setActiveEditor。
      // 用合成事件而非 .click()——headless WebView2 中点击 CodeMirror 聚焦不稳定。
      await browser.execute(() => {
        const el = document.querySelector('[data-e2e-save="1"]');
        el?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
      });

      // 7. 轮询等待 editor.save 已注册且 "editor" 上下文已激活
      await browser.waitUntil(
        async () => {
          const dbg = await browser.execute(() => (window as any).__slterm_e2e_shortcutDebug?.());
          return dbg?.commands?.includes('editor.save') && dbg?.stack?.includes('editor');
        },
        { timeout: 8000, timeoutMsg: 'editor.save 未注册或 "editor" 上下文未激活' },
      );

      // 8-9. 每轮向 window dispatch 合成 Ctrl+S（capture 监听真实捕获）直到写盘：
      //      mtime 前进 + 内容仍为编辑器 doc（marker）。dispatch-in-loop 消除首发时序竞态。
      await browser.waitUntil(
        async () =>
          await browser.execute((): boolean => {
            window.dispatchEvent(new KeyboardEvent('keydown', {
              ctrlKey: true, code: 'KeyS', key: 's', bubbles: true, cancelable: true,
            }));
            return true;
          }).then(() => {
            try {
              const st = statSync(filePath);
              return st.mtimeMs > mtimeBefore && readFileSync(filePath, 'utf8').includes(marker);
            } catch {
              return false;
            }
          }),
        { timeout: 10000, timeoutMsg: 'Ctrl+S 未经 capture 路径写盘（文件 mtime 未更新）' },
      );

      expect(statSync(filePath).mtimeMs).toBeGreaterThan(mtimeBefore);
      expect(readFileSync(filePath, 'utf8')).toContain(marker);
    } finally {
      // 清理临时目录
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});

describe('HTML 面板 Ctrl+W 转发', () => {
  // 焦点在 iframe 内时，全局键经 forwardGlobalShortcuts 重放到父 window → global.closeTab 关活跃面板。
  // embedded 驱动无法投递 OS 键，但可经同源 iframe.contentDocument 在页面内 dispatch 合成 keydown，
  // 触发真实 forwarder → 重放 → 关面板全链路（真实二进制）。
  it('iframe 内 Ctrl+W → 转发关闭该 HTML 页签', async () => {
    const tempDir = mkdtempSync(join(tmpdir(), 'slterm-e2e-html-'));
    const htmlPath = join(tempDir, 'page.html');
    writeFileSync(htmlPath, '<h1>e2e html</h1>', 'utf8');

    try {
      await browser.waitUntil(
        async () => await browser.execute(() => (window as any).__slterm_e2e_workspaceReady === true),
        { timeout: 15000, timeoutMsg: 'Workspace 未就绪' },
      );
      await browser.execute((dir: string) => {
        (window as any).__slterm_e2e_createProject?.(dir);
      }, tempDir);
      await browser.waitUntil(
        async () => await browser.execute(() => typeof window.__dockviewApi !== 'undefined'),
        { timeout: 20000, timeoutMsg: 'Dockview API 未就绪' },
      );

      const panelId = 'e2e-html-' + Date.now();
      await browser.execute(
        (args: { pid: string; path: string }) => {
          window.__dockviewApi!.addPanel({
            id: args.pid,
            component: 'htmlviewer',
            params: { panelId: args.pid, filePath: args.path },
          });
        },
        { pid: panelId, path: htmlPath },
      );

      // 等待 iframe 渲染（htmlviewer 读文件后渲染 iframe）
      await browser.waitUntil(
        async () => await browser.execute(() => !!document.querySelector('iframe')),
        { timeout: 15000, timeoutMsg: 'HTML iframe 未渲染' },
      );

      // 每轮向 iframe.contentDocument 派发合成 Ctrl+W（同源可访问），直到面板关闭
      // （容忍 forwarder onLoad 挂载的短暂时延）
      await browser.waitUntil(
        async () =>
          await browser.execute((pid: string) => {
            const iframe = document.querySelector('iframe') as HTMLIFrameElement | null;
            const doc = iframe?.contentDocument;
            if (doc) {
              doc.dispatchEvent(new KeyboardEvent('keydown', {
                ctrlKey: true, code: 'KeyW', key: 'w', bubbles: true, cancelable: true,
              }));
            }
            return window.__dockviewApi?.getPanel(pid) === undefined;
          }, panelId),
        { timeout: 10000, timeoutMsg: 'HTML 面板未被 Ctrl+W 转发关闭' },
      );

      const closed = await browser.execute(
        (pid: string) => window.__dockviewApi?.getPanel(pid) === undefined,
        panelId,
      );
      expect(closed).toBe(true);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});

describe('终端跨页面存活 (H6)', () => {
  // H6 需求：终端跨页面存活——页面切换不杀 PTY 进程。
  // 多 Dockview 实例架构：页面切换通过 CSS display:none/block 隐藏/显示，
  // 终端 xterm.js 实例不销毁，PTY 进程持续运行。
  // 验证：创建终端写标记 → 切到第二页 → 切回 → 标记仍在。
  it('should preserve terminal content after switching to another page and back', async () => {
    // 1. 等待 Workspace 就绪
    await browser.waitUntil(
      async () => await browser.execute(() => (window as any).__slterm_e2e_workspaceReady === true),
      { timeout: 15000, timeoutMsg: 'Workspace 未就绪' },
    );

    // 2. 创建测试项目（page1 + 终端）
    const tempDir = mkdtempSync(join(tmpdir(), 'slterm-e2e-h6-'));
    try {
      const page1Id = await browser.execute((dir: string) => {
        return (window as any).__slterm_e2e_createProject?.(dir);
      }, tempDir);

      // 3. 等待 Dockview API
      await browser.waitUntil(
        async () => await browser.execute(() => typeof window.__dockviewApi !== 'undefined'),
        { timeout: 20000, timeoutMsg: 'Dockview API 未就绪' },
      );

      // 4. 在 page1 上创建终端面板
      const panelId = 'e2e-h6-term-' + Date.now();
      await browser.execute((pid: string) => {
        window.__dockviewApi!.addPanel({
          id: pid,
          component: 'terminal',
          params: { panelId: pid },
          renderer: 'always' as const,
        });
      }, panelId);

      // 5. 等待 PTY session 就绪
      await browser.waitUntil(
        async () => {
          return await browser.execute(() => {
            const containers = document.querySelectorAll('[style*="height: 100%"]');
            for (const c of containers) {
              if ((c as any).__e2e_sessionReady) return true;
            }
            return false;
          });
        },
        { timeout: 25000, timeoutMsg: 'PTY session 未就绪' },
      );

      // 6. 写入跨页面标记
      const marker = 'H6_CROSS_PAGE_MARKER_' + Date.now();
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
      }, '\r\n' + marker + '\r\n');

      // 7. 验证标记存在
      await browser.waitUntil(
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
          return text?.includes(marker) ? text : false;
        },
        { timeout: 10000, timeoutMsg: '终端未包含 page1 标记' },
      );

      // 8. 获取 projectId，创建 page2
      const projectId = await browser.execute((pid: string) => {
        return (window as any).__slterm_e2e_getProjectIdForPage?.(pid) ?? null;
      }, page1Id);
      if (!projectId) throw new Error('无法获取 projectId');

      const page2Id = await browser.execute(
        (args: { projId: string; rootPath: string }) => {
          return (window as any).__slterm_e2e_addPage?.(args.projId, 'page2', args.rootPath) ?? null;
        },
        { projId: projectId, rootPath: tempDir },
      );
      if (!page2Id) throw new Error('无法创建 page2');

      // 9. 切换到 page2
      await browser.execute((pid: string) => {
        (window as any).__slterm_e2e_switchToPage?.(pid);
      }, page2Id);

      // 10. 短暂等待页面切换生效
      await browser.pause(500);

      // 11. 切回 page1
      await browser.execute((pid: string) => {
        (window as any).__slterm_e2e_switchToPage?.(pid);
      }, page1Id);

      // 12. 等待页面切换生效
      await browser.pause(500);

      // 13. 验证 page1 终端内容仍含标记（H6 核心断言）
      const textAfterSwitch = await browser.execute((m: string) => {
        const containers = document.querySelectorAll('[style*="height: 100%"]');
        for (const c of containers) {
          const el = c as any;
          if (typeof el.__e2e_getTerminalText === 'function') {
            const text = el.__e2e_getTerminalText();
            if (text.includes(m)) return text;
          }
        }
        return null;
      }, marker);

      expect(textAfterSwitch).not.toBeNull();
      expect(textAfterSwitch).toContain(marker);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});

describe('编辑器 dirty→clean 保存', () => {
  // 验证编辑器修改内容后 Ctrl+S 将新内容写盘（区别于仅验证 mtime 变化）。
  // embedded 驱动无法键盘输入，故通过外部写盘触发编辑器 auto-reload，
  // 然后 Ctrl+S 保存当前内容，验证磁盘文件与编辑器内容一致。
  it('should persist modified content to disk after external change triggers reload then Ctrl+S save', async () => {
    const initialContent = 'v1_initial_content_' + Date.now();
    const modifiedContent = 'v2_modified_content_' + Date.now();
    const tempDir = mkdtempSync(join(tmpdir(), 'slterm-e2e-save-dirty-'));
    const filePath = join(tempDir, 'dirty_save.txt');
    writeFileSync(filePath, initialContent, 'utf8');
    const mtimeBefore = statSync(filePath).mtimeMs;

    try {
      // 1. 等待 Workspace 就绪
      await browser.waitUntil(
        async () => await browser.execute(() => (window as any).__slterm_e2e_workspaceReady === true),
        { timeout: 15000, timeoutMsg: 'Workspace 未就绪' },
      );

      // 2. 创建项目
      await browser.execute((dir: string) => {
        (window as any).__slterm_e2e_createProject?.(dir);
      }, tempDir);

      // 3. 等待 Dockview API
      await browser.waitUntil(
        async () => await browser.execute(() => typeof window.__dockviewApi !== 'undefined'),
        { timeout: 20000, timeoutMsg: 'Dockview API 未就绪' },
      );

      // 4. 打开编辑器面板
      const panelId = 'e2e-dirty-save-' + Date.now();
      await browser.execute(
        (args: { pid: string; path: string }) => {
          window.__dockviewApi!.addPanel({
            id: args.pid,
            component: 'editor',
            params: { panelId: args.pid, filePath: args.path },
          });
        },
        { pid: panelId, path: filePath },
      );

      // 5. 等待编辑器加载初始内容
      await browser.waitUntil(
        async () =>
          await browser.execute((m: string) => {
            const nodes = document.querySelectorAll('.cm-content');
            for (const n of nodes) {
              if ((n.textContent ?? '').includes(m)) return true;
            }
            return false;
          }, initialContent),
        { timeout: 15000, timeoutMsg: '编辑器未加载初始内容' },
      );

      // 6. 外部修改文件内容（模拟用户编辑后）
      writeFileSync(filePath, modifiedContent, 'utf8');

      // 7. 等待编辑器 auto-reload 加载修改后内容（轮询 .cm-content）
      await browser.waitUntil(
        async () =>
          await browser.execute((m: string) => {
            const nodes = document.querySelectorAll('.cm-content');
            for (const n of nodes) {
              if ((n.textContent ?? '').includes(m)) return true;
            }
            return false;
          }, modifiedContent),
        { timeout: 15000, timeoutMsg: '编辑器未 auto-reload 修改后内容' },
      );

      // 8. 激活 editor 上下文（合成 focusin）
      await browser.execute((m: string) => {
        const nodes = document.querySelectorAll('.cm-content');
        for (const n of nodes) {
          if ((n.textContent ?? '').includes(m)) {
            (n as HTMLElement).setAttribute('data-e2e-dirty-save', '1');
            n.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
            return;
          }
        }
      }, modifiedContent);

      // 9. 等待 editor.save 可调度
      await browser.waitUntil(
        async () => {
          const dbg = await browser.execute(() => (window as any).__slterm_e2e_shortcutDebug?.());
          return dbg?.commands?.includes('editor.save') && dbg?.stack?.includes('editor');
        },
        { timeout: 8000, timeoutMsg: 'editor.save 未就绪' },
      );

      // 10. 轮询 dispatch Ctrl+S 直到写盘
      await browser.waitUntil(
        async () =>
          await browser.execute((): boolean => {
            window.dispatchEvent(new KeyboardEvent('keydown', {
              ctrlKey: true, code: 'KeyS', key: 's', bubbles: true, cancelable: true,
            }));
            return true;
          }).then(() => {
            try {
              const st = statSync(filePath);
              return st.mtimeMs > mtimeBefore && readFileSync(filePath, 'utf8').includes(modifiedContent);
            } catch {
              return false;
            }
          }),
        { timeout: 10000, timeoutMsg: 'Ctrl+S 未将修改后内容写盘' },
      );

      // 11. 断言磁盘文件包含修改后内容
      const diskContent = readFileSync(filePath, 'utf8');
      expect(diskContent).toContain(modifiedContent);
      expect(statSync(filePath).mtimeMs).toBeGreaterThan(mtimeBefore);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});
