import path from 'node:path';

export const config = {
  runner: 'local',
  specs: ['./tests/e2e/all.e2e.js'],
  maxInstances: 1,
  maxInstancesPerCapability: 1,
  workers: 1,
  framework: 'mocha',
  reporters: ['spec'],
  services: [[ '@wdio/tauri-service', {
    driverProvider: 'embedded',
    appBinaryPath: path.resolve('src-tauri/target/debug/gazeguard'),
    captureBackendLogs: true,
    captureFrontendLogs: true,
  }]],
  capabilities: [{ maxInstances: 1, browserName: 'tauri', 'tauri:options': {
    application: path.resolve('src-tauri/target/debug/gazeguard'),
  }}],
  mochaOpts: { ui: 'bdd', timeout: 120000 },
  connectionRetryTimeout: 120000,
  connectionRetryCount: 2,
};
