import path from 'node:path';

process.env.GAZEGUARD_E2E ??= '1';

const appBinaryPath = path.resolve(
  'src-tauri/target/debug',
  process.platform === 'win32' ? 'gazeguard.exe' : 'gazeguard',
);
const driverProvider = process.platform === 'win32' ? 'external' : 'embedded';

export const config = {
  runner: 'local',
  specs: ['./tests/e2e/all.e2e.js'],
  maxInstances: 1,
  maxInstancesPerCapability: 1,
  workers: 1,
  framework: 'mocha',
  reporters: ['spec'],
  services: [[ '@wdio/tauri-service', {
    driverProvider,
    autoInstallTauriDriver: driverProvider === 'external',
    appBinaryPath,
    captureBackendLogs: true,
    captureFrontendLogs: true,
  }]],
  capabilities: [{ maxInstances: 1, browserName: 'tauri', 'tauri:options': {
    application: appBinaryPath,
  }}],
  mochaOpts: { ui: 'bdd', timeout: 120000 },
  connectionRetryTimeout: 120000,
  connectionRetryCount: 2,
};
