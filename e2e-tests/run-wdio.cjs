/**
 * WDIO 兼容启动器：Node 26 的 undici 8 与 webdriverio 不兼容，
 * 自动下载便携 Node 22 运行。CI 环境（Node 22）直接运行。
 */
const { execSync, spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');
const https = require('https');

// ── settings.json 备份/还原（FIX-TE-04） ──
// E2E 运行期间侧栏视图状态等持久化到 ~/.slterminal/settings.json，
// 为免污染用户真实配置：启动时备份，进程退出时同步还原。
const settingsPath = path.join(os.homedir(), '.slterminal', 'settings.json');
const settingsBakPath = settingsPath + '.e2e-bak';
const settingsExisted = fs.existsSync(settingsPath);
if (settingsExisted) {
  fs.copyFileSync(settingsPath, settingsBakPath);
  console.log('[wdio-launcher] 已备份 settings.json → settings.json.e2e-bak');
} else {
  console.log('[wdio-launcher] settings.json 不存在，跳过备份');
}

process.on('exit', () => {
  if (settingsExisted) {
    // 原文件存在 → 用备份覆盖 E2E 运行产物
    try { fs.renameSync(settingsBakPath, settingsPath); } catch { /* 忽略 */ }
  } else {
    // 原文件不存在 → 删除 E2E 运行期间产生的 settings.json + 残留 bak
    try { fs.rmSync(settingsPath, { force: true }); } catch { /* 忽略 */ }
    try { fs.rmSync(settingsBakPath, { force: true }); } catch { /* 忽略 */ }
  }
});

const major = parseInt(process.version.slice(1).split('.')[0], 10);
const wdioConfig = path.resolve(__dirname, 'wdio.conf.ts');

function runWdio(nodeBin) {
  const wdioCli = path.resolve(__dirname, '..', 'node_modules', '@wdio', 'cli', 'bin', 'wdio.js');
  try {
    execSync(`"${nodeBin}" "${wdioCli}" run "${wdioConfig}"`, { stdio: 'inherit' });
    return true;
  } catch (e) {
    process.exit(e.status || 1);
  }
}

if (major >= 26) {
  const nodeDir = path.resolve(__dirname, '..', '.temp', 'node22');
  const node22 = path.join(nodeDir, 'node.exe');

  if (fs.existsSync(node22)) {
    console.log(`[wdio-launcher] Node ${process.version} → 使用便携 Node 22`);
    runWdio(node22);
    process.exit(0);
  }

  // 自动下载便携 Node 22
  console.log('[wdio-launcher] 下载便携 Node 22 (约 30MB)...');
  fs.mkdirSync(nodeDir, { recursive: true });

  const url = 'https://nodejs.org/dist/v22.21.1/win-x64/node.exe';
  const file = fs.createWriteStream(node22);
  https.get(url, (res) => {
    if (res.statusCode === 302 || res.statusCode === 301) {
      https.get(res.headers.location, (r2) => r2.pipe(file));
    } else {
      res.pipe(file);
    }
    file.on('finish', () => {
      file.close();
      console.log('[wdio-launcher] Node 22 就绪，启动 WDIO...');
      runWdio(node22);
    });
  }).on('error', (err) => {
    fs.unlink(node22, () => {});
    console.error('[wdio-launcher] 下载失败:', err.message);
    console.warn('[wdio-launcher] 尝试用当前 Node 运行（可能因 undici 8 失败）');
    fallback();
  });
} else {
  fallback();
}

function fallback() {
  const wdio = spawn('npx', ['wdio', 'run', wdioConfig], {
    stdio: 'inherit',
    shell: true,
  });
  wdio.on('close', (code) => process.exit(code));
}
