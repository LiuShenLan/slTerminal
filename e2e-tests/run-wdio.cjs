/**
 * WDIO 兼容启动器：Node 26 的 undici 8 与 webdriverio 不兼容，
 * 自动下载便携 Node 22 运行。CI 环境（Node 22）直接运行。
 *
 * 备份/还原（E2E-05 扩展）：E2E 运行会真实写盘三处用户配置——
 * ① ~/.slterminal/settings.json（侧栏视图状态等，FIX-TE-04 原有）
 * ② ~/.claude/settings.json（hooks_inject 注入 slterm matcher，E2E-05 新增）
 * ③ ~/.slterminal/hooks/（注入的 reporter 脚本，E2E-05 新增）
 * 启动时备份（存在时），exit 时同步还原；~/.slterminal/hooks-events/
 * 为运行时信号文件目录（无用户价值），exit 时直接清理。
 * 三启动路径（node22 直跑 / 便携下载 / fallback）均在同一主进程内
 * exit——单一 process.on('exit') 钩子天然全覆盖。
 */
const { execSync, spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');
const https = require('https');

// ── 配置备份/还原（FIX-TE-04 + E2E-05） ──

/** 备份单个文件：存在时复制为 .e2e-bak，返回是否备份 */
function backupFile(filePath) {
  const bakPath = filePath + '.e2e-bak';
  if (fs.existsSync(filePath)) {
    fs.copyFileSync(filePath, bakPath);
    console.log(`[wdio-launcher] 已备份 ${filePath} → .e2e-bak`);
    return true;
  }
  console.log(`[wdio-launcher] ${filePath} 不存在，跳过备份`);
  return false;
}

/** 还原单个文件：原存在 → 先删产物再 rename 备份回来；原不存在 → 删产物 + 残留 bak */
function restoreFile(filePath, existed) {
  const bakPath = filePath + '.e2e-bak';
  // E2E-13②：还原前先 rmSync 原路径（防 E2E 期间文件被删/损坏导致 rename/copy 失败、
  // 残留 bak 影响下次运行判定），再 rename 备份回来（同卷原子移动）
  try { fs.rmSync(filePath, { force: true }); } catch { /* 忽略 */ }
  if (existed) {
    try { fs.renameSync(bakPath, filePath); } catch { /* 忽略 */ }
  } else {
    try { fs.rmSync(bakPath, { force: true }); } catch { /* 忽略 */ }
  }
}

const settingsPath = path.join(os.homedir(), '.slterminal', 'settings.json');
const settingsExisted = backupFile(settingsPath);

// ~/.claude/settings.json（hooks 注入污染防护，E2E-05）
const claudeSettingsPath = path.join(os.homedir(), '.claude', 'settings.json');
const claudeSettingsExisted = backupFile(claudeSettingsPath);

// ~/.slterminal/hooks/（reporter 脚本目录，E2E-05）：
// 用户可能已有注入——目录整体备份/还原（cpSync 复制，rename 对占用目录易失败）；
// 原本不存在时 exit 删除 E2E 注入产物。
const hooksDir = path.join(os.homedir(), '.slterminal', 'hooks');
const hooksDirBak = hooksDir + '.e2e-bak';
const hooksExisted = fs.existsSync(hooksDir);
let hooksBackedUp = false;
if (hooksExisted) {
  try {
    fs.cpSync(hooksDir, hooksDirBak, { recursive: true });
    hooksBackedUp = true;
    console.log('[wdio-launcher] 已备份 ~/.slterminal/hooks/ → hooks.e2e-bak');
  } catch (err) {
    // 备份失败（目录被占用等）：降级——exit 时不动用户目录，仅清 bak
    console.warn('[wdio-launcher] hooks 目录备份失败，exit 时跳过还原:', err.message);
  }
}

// ~/.slterminal/hooks-events/（信号文件运行时目录）：不做备份，exit 时直接清理
const hooksEventsDir = path.join(os.homedir(), '.slterminal', 'hooks-events');

process.on('exit', () => {
  // settings.json 还原（FIX-TE-04）
  restoreFile(settingsPath, settingsExisted);
  // ~/.claude/settings.json 还原（E2E-05——hooks_inject 会真实写 slterm matcher）
  restoreFile(claudeSettingsPath, claudeSettingsExisted);
  // hooks-events 清理（E2E-05——信号文件目录，watcher 消费后残留兜底删除）
  try { fs.rmSync(hooksEventsDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
  // hooks 目录还原/清理（E2E-05）
  if (hooksBackedUp) {
    // 还原：删 E2E 产物 → 从备份复制回来 → 清 bak
    try { fs.rmSync(hooksDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    try { fs.cpSync(hooksDirBak, hooksDir, { recursive: true }); } catch { /* 忽略 */ }
    try { fs.rmSync(hooksDirBak, { recursive: true, force: true }); } catch { /* 忽略 */ }
  } else if (hooksExisted) {
    // 备份失败降级：无法还原原状——保留用户目录（日志已 warn），仅清 bak
    try { fs.rmSync(hooksDirBak, { recursive: true, force: true }); } catch { /* 忽略 */ }
  } else {
    // 原本不存在 → 删除 E2E 注入产物 + 残留 bak
    try { fs.rmSync(hooksDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    try { fs.rmSync(hooksDirBak, { recursive: true, force: true }); } catch { /* 忽略 */ }
  }
});

// ── Claude 历史会话 fixture 副本 + env 注入（TE-02，SEC-02 安全红线） ──
// 后端 claude_history 扫描根支持 SLTERM_CLAUDE_PROJECTS_DIR env 覆盖（SEC-02/BE-06）。
// 每次运行从 fixtures/claude-projects/ 重建 e2e-tests/.tmp-claude-projects/ 副本
// （防用例间污染；删除用例只动副本，不触碰用户真实 ~/.claude/projects/）。
// 复制时替换占位符 __E2E_PROJECT_DIR__ 为 E2E 临时项目目录真实绝对路径
// （JSON 字符串内反斜杠须转义为 \\，保证替换后 JSON 合法）。
// fixture 维护说明见 fixtures/claude-projects/README.md（E2E-13③）。
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

  // E2E-13①：便携 Node 22 预置 .temp/node22 或 CI 固定 Node 22 时跳过外网下载。
  // 判活：文件存在且大小 > 1MB（防下载中断残留的空/损坏文件被误判可用）
  if (fs.existsSync(node22)) {
    let size = 0;
    try { size = fs.statSync(node22).size; } catch { size = 0; }
    if (size > 1024 * 1024) {
      console.log(`[wdio-launcher] Node ${process.version} → 使用便携 Node 22`);
      runWdio(node22);
      process.exit(0);
    }
    console.warn('[wdio-launcher] 便携 Node 22 文件不完整（<1MB），重新下载');
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
