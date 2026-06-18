/// <reference types="@wdio/tauri-service" />

export const config: WebdriverIO.Config = {
  runner: 'local',
  tsConfigPath: './test/tsconfig.json',

  specs: ['./test/specs/**/*.ts'],

  maxInstances: 1,

  capabilities: [{
    maxInstances: 1,
    'tauri:options': {
      application: './src-tauri/target/debug/slterminal.exe',
    },
  }],

  logLevel: 'info',
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  services: [[
    'tauri',
    {
      driverProvider: 'official',
    },
  ], 'tauri-plugin'],

  framework: 'mocha',

  reporters: ['spec'],

  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
  },
};
