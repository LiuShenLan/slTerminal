// scripts/win32_input_matrix.ps1 的探针侧——作为 PTY 子进程跑在 slterm 内,
// 把每个 stdin 块以 hex 落 log。驱动方注入固定键矩阵(PostMessage)并对
// 拼接后的字节序列断言入库基线。收到 'q'(0x71)即退出。
const fs = require('fs');
const out = process.argv[2];
if (!out) { console.error('usage: node win32_input_matrix_probe.cjs <log-path>'); process.exit(2); }
const t0 = Date.now();
const log = (s) => fs.appendFileSync(out, `${((Date.now() - t0) / 1000).toFixed(1)}s ${s}\n`);
fs.writeFileSync(out, `matrix-probe ready node=${process.version} isTTY=${process.stdin.isTTY}\n`);
process.stdin.setRawMode(true);
process.stdin.resume();
process.stdin.on('data', (b) => {
  log(`rx ${b.toString('hex')}`);
  if (b.includes(0x71)) { log('end'); process.exit(0); }
});
setTimeout(() => { log('timeout'); process.exit(0); }, 90000);
