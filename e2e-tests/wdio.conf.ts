/**
 * L4 E2E 配置 — slTerminal
 *
 * 使用 @wdio/tauri-service + driverProvider: 'embedded'，Tauri 内嵌 WebDriver 直接驱动 WebView2。
 * 本地用 Node 22 便携版自动切换（Node 26 undici 8 与 webdriverio 不兼容），CI 固定 Node 22。
 */
import type { TauriDriverOptions } from '@wdio/tauri-plugin';

export const config: WebdriverIO.Config = {
  runner: 'local',

  specs: ['./test.e2e.ts'],

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
  },
};
