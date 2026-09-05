/**
 * WDIO 兼容启动器：Node 26 的 undici 8 与 webdriverio 不兼容，
 * 自动下载便携 Node 22 运行。CI 环境（Node 22）直接运行。
 *
 * 数据隔离（BE-01/TE-02）：应用全部数据写入（settings.json / 项目持久化文件
 * 等）经 SLTERM_DATA_DIR 指向 os.tmpdir()/slterm-e2e-data 临时目录，与日常使用
 * 数据完全隔离。临时目录启动时清空重建，exit 时删除。
 *
 * 假 home 隔离（ADR-0016，替代原 E2E-05 备份/还原机制）：E2E 运行会真实写盘
 * 用户 home 配置（~/.claude/settings.json 的 hooks matcher/statusLine 桥接/假 env、
 * ~/.slterminal/hooks 脚本等——窗口期污染真实 claude 会话曾致 API token 事故），
 * 备份/还原只能保证 run 后恢复、窗口期与残留固化均无法消除。改为：启动时建
 * 临时假 home（os.tmpdir()/slterm-e2e-home）并把 USERPROFILE 指向它——Node
 * os.homedir()（libuv，每调重读）与 Rust 侧 crate::home 共享解析（env-first）
 * 全链跟随，e2e 全部用户目录写入落假屋，真实用户目录零接触。
 *
 * 防复发校验：覆盖 USERPROFILE 前对真实屋（~/.claude/settings.json、
 * ~/.slterminal/statusline-backup.json、~/.slterminal/hooks/）做存在性 + sha256
 * 快照，exit 时逐项比对——任何泄漏（Rust 侧收敛遗漏/未来新消费点裸 dirs）都会
 * 在退出时独立报红（exitCode=1，TQ-E-06 可观测纪律）。hooks-events 目录仅在
 * 启动时不存在才校验「exit 仍不存在」（存在 = 用户会话在用，跳过防误报）。
 */
const { execSync, spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');
const https = require('https');
const crypto = require('crypto');

// ── E2E 数据目录隔离（BE-01/TE-02） ──
// 应用全部数据写入（settings.json / 项目持久化文件等）经 SLTERM_DATA_DIR
// 指向临时目录，与日常使用数据完全隔离。位置必须在 spawn wdio 之前——
// env 链式继承：run-wdio → npx wdio → tauri driver → slterminal.exe。
const e2eDataDir = path.join(os.tmpdir(), 'slterm-e2e-data');
fs.rmSync(e2eDataDir, { recursive: true, force: true });
fs.mkdirSync(e2eDataDir, { recursive: true });
process.env.SLTERM_DATA_DIR = e2eDataDir;

// ── 假 home 隔离（ADR-0016，见文件头） ──
// 假屋目录每次运行唯一（pid 后缀）：IME/遥测组件（搜狗输入法等）经 e2e 进程链
// （WebView2 激活）拉起后继承假 USERPROFILE，会把自身 AppData 数据写进假屋且
// 长驻句柄——固定名假屋的启动清空必撞 EPERM。唯一名免清空；exit 清理 best-effort，
// 残留目录留在 tmp 由 OS 回收（无害——隔离目标已达成，真实屋零接触）。
const fakeHomeDir = path.join(os.tmpdir(), `slterm-e2e-home-${process.pid}`);

/** 忙等（exit 钩子内无法用 setTimeout——E2E-16 模式） */
function busyWait(ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) { /* 忙等 */ }
}

/** rmSync 带重试：slterminal.exe/IME 组件退出异步，假屋文件可能被句柄占用（E2E-16） */
function rmSyncRetry(p, opts) {
  for (let i = 0; i < 5; i++) {
    try {
      fs.rmSync(p, opts);
      return true;
    } catch {
      if (i < 4) busyWait(1000);
    }
  }
  return false;
}

/** 单文件快照：{ exists, sha256|null }（读失败视为不存在——文件被并发删等瞬态） */
function snapFile(p) {
  try {
    return {
      exists: true,
      sha256: crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex'),
    };
  } catch {
    return { exists: false, sha256: null };
  }
}

/** 目录树快照：相对路径 → sha256 列表（目录不存在/不可读 → null） */
function snapDir(p) {
  const out = [];
  const walk = (base, rel) => {
    for (const entry of fs.readdirSync(base, { withFileTypes: true })) {
      const abs = path.join(base, entry.name);
      const relName = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        walk(abs, relName);
      } else {
        out.push([relName, snapFile(abs).sha256 ?? '']);
      }
    }
  };
  if (!fs.existsSync(p)) return null;
  try {
    walk(p, '');
    return out;
  } catch {
    return null;
  }
}

/** 真实屋快照——必须在 USERPROFILE 覆盖之前调用（真实路径计算） */
function snapshotUserHome(realHome) {
  return {
    claudeSettings: snapFile(path.join(realHome, '.claude', 'settings.json')),
    statuslineBackup: snapFile(path.join(realHome, '.slterminal', 'statusline-backup.json')),
    hooksDir: snapDir(path.join(realHome, '.slterminal', 'hooks')),
    hooksEventsExisted: fs.existsSync(path.join(realHome, '.slterminal', 'hooks-events')),
  };
}

/** exit 校验：真实屋零接触。返回差异描述数组（空 = 通过） */
function verifyRealHomeUnchanged(realHome, snap) {
  const problems = [];
  const expectFile = (label, cur, before) => {
    if (JSON.stringify(cur) !== JSON.stringify(before)) {
      problems.push(`${label} 在 E2E 期间被修改（泄漏）——启动时 ${before.exists ? before.sha256?.slice(0, 12) ?? '存在' : '不存在'}，当前 ${cur.exists ? cur.sha256?.slice(0, 12) ?? '存在' : '不存在'}`);
    }
  };
  expectFile(
    `真实屋 ${path.join(realHome, '.claude', 'settings.json')}`,
    snapFile(path.join(realHome, '.claude', 'settings.json')),
    snap.claudeSettings,
  );
  expectFile(
    `真实屋 ${path.join(realHome, '.slterminal', 'statusline-backup.json')}`,
    snapFile(path.join(realHome, '.slterminal', 'statusline-backup.json')),
    snap.statuslineBackup,
  );
  const hooksDir = path.join(realHome, '.slterminal', 'hooks');
  const hooksNow = snapDir(hooksDir);
  if (JSON.stringify(hooksNow) !== JSON.stringify(snap.hooksDir)) {
    problems.push(`真实屋 ${hooksDir} 内容在 E2E 期间被修改（泄漏）`);
  }
  // hooks-events 降噪：启动时不存在才校验 exit 仍不存在（存在 = 用户会话在用）
  const eventsDir = path.join(realHome, '.slterminal', 'hooks-events');
  if (!snap.hooksEventsExisted && fs.existsSync(eventsDir)) {
    problems.push(`真实屋 ${eventsDir} 在 E2E 期间被创建（泄漏——E2E 信号文件必须落假屋）`);
  }
  return problems;
}

// 覆盖 USERPROFILE 之前：记录真实 home 并做零接触快照
const realHome = os.homedir();
const homeSnapshot = snapshotUserHome(realHome);

// 假屋：唯一名目录 + USERPROFILE 注入（Node os.homedir / Rust crate::home 全链跟随）
fs.mkdirSync(fakeHomeDir, { recursive: true });
process.env.USERPROFILE = fakeHomeDir;
console.log(`[wdio-launcher] 假 home 隔离: ${realHome} → ${fakeHomeDir}`);
// sanity：env 链断裂则 fail-fast（断链 = 整轮在真实屋上裸奔，静默不可接受）
if (os.homedir() !== fakeHomeDir) {
  console.error('[wdio-launcher] USERPROFILE 注入未生效（os.homedir 未跟随），E2E 终止——防在真实用户目录裸奔');
  process.exit(1);
}

process.on('exit', () => {
  // 真实屋零接触校验（防复发——任何 Rust 收敛遗漏/新裸 dirs 消费点在此独立报红）
  const problems = verifyRealHomeUnchanged(realHome, homeSnapshot);
  if (problems.length > 0) {
    console.error(`[wdio-launcher] 真实用户目录校验失败 ${problems.length} 项（疑似泄漏）:`);
    for (const p of problems) console.error(`  - ${p}`);
    console.error('[wdio-launcher] 排查方向：Rust 侧 home 解析是否收敛到 crate::home（禁裸 dirs::home_dir）');
    process.exitCode = 1;
  } else {
    console.log('[wdio-launcher] 真实用户目录校验通过（零接触）');
  }
  // E2E 临时目录清理（best-effort——IME/遥测组件句柄可能导致假屋删不净，
  // 残留于 tmp 无害且 OS 可回收；固定名 e2eDataDir 残留由下次启动 rmSync 兜底）
  rmSyncRetry(e2eDataDir, { recursive: true, force: true });
  if (!rmSyncRetry(fakeHomeDir, { recursive: true, force: true })) {
    console.warn(`[wdio-launcher] 假屋清理失败（第三方组件句柄占用）→ 残留 ${fakeHomeDir}，无害可手动删`);
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
  // fixtures 缺失（AQ-4）：E2E 终止而非降级——不设 env 会令后端回落真实
  // ~/.claude/projects（生产默认），历史会话用例有触碰真实用户目录风险
  console.error('[wdio-launcher] fixtures/claude-projects 缺失，E2E 终止——防止回落真实 ~/.claude/projects');
  process.exit(1);
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
