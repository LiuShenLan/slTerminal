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

// ── Claude 历史会话 fixture 副本 + env 注入（TE-02，SEC-02 安全红线） ──
// 后端 claude_history 扫描根支持 SLTERM_CLAUDE_PROJECTS_DIR env 覆盖（SEC-02/BE-06）。
// 每次运行从 fixtures/claude-projects/ 重建 e2e-tests/.tmp-claude-projects/ 副本
// （防用例间污染；删除/重命名用例只动副本，不触碰用户真实 ~/.claude/projects/）。
// 复制时替换占位符 __E2E_PROJECT_DIR__ 为 E2E 临时项目目录真实绝对路径
// （JSON 字符串内反斜杠须转义为 \\，保证替换后 JSON 合法）。
const fixturesDir = path.join(__dirname, 'fixtures', 'claude-projects');
const tmpProjectsDir = path.join(__dirname, '.tmp-claude-projects');
// E2E 临时项目目录：恢复编排用例的项目根（fixture cwd 占位符指向它，须真实存在 → cwdExists=true）
const e2eProjectDir = path.join(os.tmpdir(), 'slterm-e2e-history-project');

/** 递归复制 fixture 树到副本目录，占位符替换为真实路径（JSON 转义后） */
function copyFixtureTree(src, dst, placeholder, realJsonEscaped) {
  fs.rmSync(dst, { recursive: true, force: true });
  fs.mkdirSync(dst, { recursive: true });
  const walk = (from, to) => {
    fs.mkdirSync(to, { recursive: true });
    for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
      const s = path.join(from, entry.name);
      const d = path.join(to, entry.name);
      if (entry.isDirectory()) {
        walk(s, d);
      } else {
        const content = fs.readFileSync(s, 'utf8');
        fs.writeFileSync(d, content.split(placeholder).join(realJsonEscaped), 'utf8');
      }
    }
  };
  walk(src, dst);
}

// 重建 E2E 临时项目目录（固定路径，每次运行清空重建——fixture cwd 指向它）
fs.rmSync(e2eProjectDir, { recursive: true, force: true });
fs.mkdirSync(e2eProjectDir, { recursive: true });
process.env.SLTERM_E2E_PROJECT_DIR = e2eProjectDir;

if (fs.existsSync(fixturesDir)) {
  // 重建 fixture 副本 + 占位符替换（Windows 路径反斜杠 → JSON 转义 \\）
  copyFixtureTree(
    fixturesDir,
    tmpProjectsDir,
    '__E2E_PROJECT_DIR__',
    e2eProjectDir.replace(/\\/g, '\\\\'),
  );
  process.env.SLTERM_CLAUDE_PROJECTS_DIR = tmpProjectsDir;
  console.log(`[wdio-launcher] 已重建 claude-projects 副本 → ${tmpProjectsDir}`);
  console.log(`[wdio-launcher] SLTERM_CLAUDE_PROJECTS_DIR=${tmpProjectsDir}`);
  console.log(`[wdio-launcher] SLTERM_E2E_PROJECT_DIR=${e2eProjectDir}`);
} else {
  // fixtures 缺失（异常路径）：不设 env——后端回落真实 ~/.claude/projects（生产默认）；
  // 历史会话用例会失败，属显式信号而非静默污染真实数据
  console.warn('[wdio-launcher] fixtures/claude-projects 不存在，跳过 SLTERM_CLAUDE_PROJECTS_DIR 注入');
}

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
