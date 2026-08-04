/**
 * L4 E2E 配置 — slTerminal
 *
 * 使用 @wdio/tauri-service + driverProvider: 'embedded'，Tauri 内嵌 WebDriver 直接驱动 WebView2。
 * 本地用 Node 22 便携版自动切换（Node 26 undici 8 与 webdriverio 不兼容），CI 固定 Node 22。
 *
 * specs 通配（E2E-09）：按领域拆分后的 *.e2e.ts 自动纳入，新增 spec 无需改配置。
 * 同一 worker 顺序执行（maxInstances=1，单 session）——spec 间共享 app 实例，
 * 与拆分前单文件语义一致；字母序即执行序（terminal.e2e.ts 末位承载 E2E-12 杀 app 用例）。
 */
import type { TauriDriverOptions } from '@wdio/tauri-plugin';

export const config: WebdriverIO.Config = {
  runner: 'local',

  specs: ['./*.e2e.ts'],

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
    // 同一 session 内重跑失败用例（成本最低）；E2E-12 杀 app 用例在用例内
    // this.retries(0) 显式关闭（重试时 session/app 已不可用）。
    // 不用 specFileRetries（整文件重跑成本高，且与 E2E-12 杀 app 语义冲突）。
    retries: 1,
  },
};
