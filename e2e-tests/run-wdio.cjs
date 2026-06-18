/**
 * WDIO 兼容启动器：Node 26 的 undici 8 与 webdriverio 不兼容，
 * 自动切换到便携 Node 22 运行。CI 环境（Node 22）直接运行。
 */
const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const major = parseInt(process.version.slice(1).split('.')[0], 10);
const wdioConfig = path.resolve(__dirname, 'wdio.conf.ts');

if (major >= 26) {
  const node22 = path.resolve(__dirname, '..', '.temp', 'node22', 'node.exe');
  const wdioCli = path.resolve(__dirname, '..', 'node_modules', '@wdio', 'cli', 'bin', 'wdio.js');

  if (fs.existsSync(node22)) {
    console.log(`[wdio-launcher] Node ${process.version} → 使用便携 Node 22`);
    try {
      execSync(`"${node22}" "${wdioCli}" run "${wdioConfig}"`, { stdio: 'inherit' });
      process.exit(0);
    } catch (e) {
      process.exit(e.status || 1);
    }
  } else {
    console.warn('[wdio-launcher] 便携 Node 22 未找到，尝试用当前 Node 运行（可能因 undici 8 失败）');
  }
}

// Node < 26 或 便携 Node 22 不存在时，直接用当前 Node
const { spawn } = require('child_process');
const wdio = spawn('npx', ['wdio', 'run', wdioConfig], {
  stdio: 'inherit',
  shell: true,
});
wdio.on('close', (code) => process.exit(code));
