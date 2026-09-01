/**
 * L4 E2E 配置 — slTerminal
 *
 * 使用 @wdio/tauri-service + driverProvider: 'embedded'，Tauri 内嵌 WebDriver 直接驱动 WebView2。
 * 本地用 Node 22 便携版自动切换（Node 26 undici 8 与 webdriverio 不兼容），CI 固定 Node 22。
 *
 * specs 显式数组（E2E-09）：新增 spec 须加入下方显式数组。
 * 同一 worker 顺序执行（maxInstances=1，单 session）——spec 间共享 app 实例，
 * 与拆分前单文件语义一致；字母序即执行序（terminal.e2e.ts 末位承载 E2E-12 杀 app 用例）。
 */
import type { TauriDriverOptions } from '@wdio/tauri-plugin';

export const config: WebdriverIO.Config = {
  runner: 'local',

  specs: [
    './agent.e2e.ts',
    './background-tasks.e2e.ts',
    './commit.e2e.ts',
    './editor.e2e.ts',
    './history.e2e.ts',
    './hooks.e2e.ts',
    './html.e2e.ts',
    './mockcli.e2e.ts',
    './settings.e2e.ts',
    './sidebar.e2e.ts',
    './terminal.e2e.ts',
  ],

  maxInstances: 1,

  hostname: '127.0.0.1',
  port: 4445,

  capabilities: [{
    browserName: 'tauri',
    'tauri:options': {
      application: './src-tauri/target/debug/slterminal.exe',
    } satisfies TauriDriverOptions,
  }],

  services: [['tauri', {
    driverProvider: 'embedded',
  }]],

  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
    // E2E-15：用例级重试（mocha --retries），单条 flaky 不拖垮整轮。
    // 默认 1；WDIO_RETRIES=0 时关闭——CI 观察面 job（e2e-flakiness-probe）用它暴露
    // 真实 flakiness（TQ-E-09）。同一 session 内重跑失败用例（成本最低）；E2E-12 杀 app
    // 用例在用例内 this.retries(0) 显式关闭（重试时 session/app 已不可用）。
    // 不用 specFileRetries（整文件重跑成本高，且与 E2E-12 杀 app 语义冲突）。
    retries: Number(process.env.WDIO_RETRIES ?? "1"),
  },

  // 每个 spec 开始前清空项目 store——单 session 共享 app 实例（见文件头注释），
  // 前序 spec 的项目在 store 累积（一轮可 20+ 项目/30+ 页），S06 FE-36 全局
  // 页数上限（MAX_PAGES=20）会拒绝后续 addPage（H6/E2E-04 回归根因）。
  // 粒度 = spec 级（beforeSuite 先于 mocha before()——后者建的项目不被清；
  // 不用 beforeTest：wdio 层在 mocha before() 之后执行，会清掉 before()
  // 里建的项目，且 editor 标题等用例依赖 spec 内累积状态）。
  // spec 内用例累积 ≤10 项目不触发 20 页上限；用例内多项目（agent R2）不受影响。
  beforeSuite: async function () {
    await browser.execute(() => {
      const w = window as unknown as {
        __slterm_e2e_resetProjects?: () => void;
        __slterm_e2e_resetSettings?: () => void;
      };
      w.__slterm_e2e_resetProjects?.();
      // TQ-E-08：settings 类 store（keybindings/sideBar/fontSize 内存态）同步隔离——
      // 防前序 spec 的用户覆盖/侧栏形态/字号跨 spec 泄漏；后端 settings.json 由
      // run-wdio.cjs 备份/还原兜底，此处只管同一 run 内的 Zustand 内存态
      w.__slterm_e2e_resetSettings?.();
    });
  },
};
