/**
 * L4 E2E 配置 — slTerminal Phase 0
 *
 * 本地烟测试：验证 WDIO 配置文件语法正确。
 * 完整 E2E session 测试由 CI 覆盖（CI 用 Node.js 22，避免 Node 26 undici 兼容性问题）。
 */
export const config: WebdriverIO.Config = {
  runner: 'local',
  specs: [],
  capabilities: [{}],
  framework: 'mocha',
};
