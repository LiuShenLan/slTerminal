// statusline-bridge-behavior.test.ts — 桥接脚本行为级测试（B11，L2）
//
// 填补「无任何测试真正执行桥接脚本 JS」的盲区：读 src-tauri/src/hooks/claude/
// slterm-statusline.js 源码 + spawnSync 真实 node 执行，验证透传分支（.sh 引号容忍 /
// 系统 shell / 失败占位）、信号文件 payload、节流抑制与 C10 契约（exit 0 + stderr 空）。
//
// 隔离纪律：每用例 mkdtemp 独立临时 HOME（节流状态文件跨进程共享，共目录会串扰）；
// node 侧 os.homedir() 在 Windows 读 USERPROFILE、POSIX 读 HOME——双设覆盖。
// bash 缺失时 .sh 用例 it.skip（Git Bash 是开发机既有依赖，非 CI 硬约束）。

import { describe, it, expect, beforeAll } from "vitest";
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

const SCRIPT_PATH = path.resolve(
  process.cwd(),
  "src-tauri/src/hooks/claude/slterm-statusline.js",
);

/** 失败占位文案（与脚本内字面量一致——断言漂移即实现有误） */
const PLACEHOLDER = "[slterm-statusline: 命令执行失败]";

/** 运行桥接脚本（真实 node 执行，独立 HOME 注入）。
 *  脚本须拷贝到临时目录执行——仓库 package.json "type": "module" 会使 node 按
 *  ESM 解析仓库内 .js（require 未定义）；生产环境脚本落盘于 ~/.slterminal/hooks/
 *  （无 package.json 影响），拷贝执行即还原生产 CommonJS 语义。 */
function runBridge(opts: {
  home: string;
  panelId?: string;
  input?: string;
  args?: string[];
  /** 覆盖子进程 PATH（B16 用例：模拟生产 Windows 原生 PATH——无 bash 仅 Git\cmd） */
  pathOverride?: string;
}) {
  const scriptCopy = path.join(opts.home, "slterm-statusline.js");
  fs.copyFileSync(SCRIPT_PATH, scriptCopy);
  const env: Record<string, string | undefined> = {
    ...process.env,
    HOME: opts.home,
    USERPROFILE: opts.home,
    SLTERM_PANEL_ID: opts.panelId,
  };
  if (opts.panelId === undefined) delete env.SLTERM_PANEL_ID;
  if (opts.pathOverride !== undefined) {
    delete env.Path; // Windows 键名双写——Node 大小写不敏感，显式删除防残留
    env.PATH = opts.pathOverride;
  }
  return spawnSync(process.execPath, [scriptCopy, ...(opts.args ?? [])], {
    input: opts.input ?? "",
    encoding: "utf8",
    env,
    windowsHide: true,
  });
}

/** 独立临时 HOME（用例结束自动清理由 tempdir 语义保证——测试进程退出后残留无害） */
function makeHome(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), "slterm-bridge-"));
}

/** 列出 HOME 下 hooks-events 目录的 .json 信号文件 */
function listSignals(home: string): string[] {
  const dir = path.join(home, ".slterminal", "hooks-events");
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir).filter((f) => f.endsWith(".json"));
}

/** 生产形态 Git\cmd 目录（where git 输出中含 \cmd\git.exe 的行；非标准布局 → null 该用例 skip）。
 *  采集于模块级（收集期判定），与 bashReachable 同为用例 skip 条件 */
const gitCmdDir = (() => {
  const w = spawnSync("where", ["git"], { encoding: "utf8", windowsHide: true });
  if (w.error || !w.stdout) return null;
  const line = w.stdout
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean)
    .find((l) => /[\\/]cmd[\\/]git(\.exe)?$/.test(l));
  return line ? path.dirname(line) : null;
})();

/** bash 可达性（B16）：PATH 的 bash，或 where git 推导（沿目录上溯 3 层探 bin/usr\bin）——
 *  与脚本内 bashCandidates 的推导逻辑一致（断言漂移即实现有误） */
const bashReachable = (() => {
  const r = spawnSync("bash", ["--version"], { windowsHide: true });
  if (!r.error) return true;
  const w = spawnSync("where", ["git"], { encoding: "utf8", windowsHide: true });
  if (w.error || !w.stdout) return false;
  const line = w.stdout
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean)[0];
  if (!line) return false;
  let dir = path.dirname(line);
  for (let hops = 0; hops < 3 && dir; hops++) {
    if (
      fs.existsSync(path.join(dir, "bin", "bash.exe")) ||
      fs.existsSync(path.join(dir, "usr", "bin", "bash.exe"))
    ) {
      return true;
    }
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return false;
})();

describe("statusline 桥接脚本行为（B11/B16，真实 node 执行）", () => {
  beforeAll(() => {
    expect(fs.existsSync(SCRIPT_PATH), `桥接脚本不存在: ${SCRIPT_PATH}`).toBe(true);
  });

  // B16：skip 条件升级——PATH 无 bash 时经 git 推导仍可跑（生产 Windows 原生 PATH 常态）；
  // 断言旧实现必然失败（bash ENOENT → 占位 ≠ marker），锁死 git 推导 + 正斜杠链路
  (bashReachable ? it : it.skip)(".sh 尾随引号容忍：剥引号后 ~ 展开 + bash 分支（防 cmd.exe 失配）", () => {
    const home = makeHome();
    // 建用户原 .sh 脚本（echo marker）
    const shDir = path.join(home, ".claude");
    fs.mkdirSync(shDir, { recursive: true });
    fs.writeFileSync(
      path.join(shDir, "statusline-deepseek.sh"),
      "echo marker-statusline-ok",
      "utf8",
    );
    // argv 带尾随引号（损坏中间态形态）——旧实现 .sh 正则失配落 cmd.exe 分支
    const r = runBridge({
      home,
      panelId: "p-behavior-1",
      input: JSON.stringify({ context_window: { used_percentage: 42 } }),
      args: ['"~/.claude/statusline-deepseek.sh"'],
    });
    expect(r.status).toBe(0);
    expect(r.stderr).toBe("");
    expect(r.stdout).toContain("marker-statusline-ok");
  });

  // B16-a：模拟生产 Windows 原生 PATH（无 bash、仅 Git\cmd + System32）——
  // 桥接脚本须经 where git 推导同根 Git\bin\bash.exe 完成透传。
  // where git 可能先命中 mingw64\bin（Git Bash 增强 PATH）——裁剪 PATH 时须取
  // 生产形态的 Git\cmd 目录（含 git.exe 不含 bash）；非标准布局（无 \cmd\ 行）跳过
  (bashReachable && gitCmdDir ? it : it.skip)("git 推导定位 bash：PATH 无 bash 时经 Git\\cmd\\git.exe 推导同根 bash", () => {
    const home = makeHome();
    const shDir = path.join(home, ".claude");
    fs.mkdirSync(shDir, { recursive: true });
    fs.writeFileSync(
      path.join(shDir, "statusline-deepseek.sh"),
      "echo marker-git-derived-bash",
      "utf8",
    );
    const system32 = path.join(process.env.SystemRoot ?? "C:\\Windows", "System32");
    const trimmedPath = [gitCmdDir, system32].join(path.delimiter);
    // 前提自检：裁剪 PATH 中无 bash（Git\cmd 不含 bash；否则用例失去意义）
    expect(trimmedPath.toLowerCase()).not.toContain("usr\\bin");
    expect(trimmedPath.toLowerCase()).not.toContain("git\\bin");

    const r = runBridge({
      home,
      panelId: "p-behavior-git",
      input: JSON.stringify({ context_window: { used_percentage: 42 } }),
      args: ['"~/.claude/statusline-deepseek.sh"'],
      pathOverride: trimmedPath,
    });
    expect(r.status).toBe(0);
    expect(r.stderr).toBe("");
    expect(r.stdout).toContain("marker-git-derived-bash");
  });

  // B16-a 兜底：bash 完全不可得（PATH 无 bash 无 git、固定路径不存在）→ 占位文本可见。
  // 机器装有默认安装位 Git 时跳过——固定路径是脚本内硬编码 fallback，该环境下
  // 「完全不可得」不可构造（bash 恒可达属预期行为）
  const defaultGitExists =
    fs.existsSync("C:\\Program Files\\Git\\bin\\bash.exe") ||
    fs.existsSync("C:\\Program Files (x86)\\Git\\bin\\bash.exe");
  (defaultGitExists ? it.skip : it)("bash 完全不可得 → stdout 占位文本（C10 保持：stderr 空 + exit 0）", () => {
    const home = makeHome();
    const shDir = path.join(home, ".claude");
    fs.mkdirSync(shDir, { recursive: true });
    fs.writeFileSync(
      path.join(shDir, "statusline-deepseek.sh"),
      "echo marker-never-runs",
      "utf8",
    );
    const system32 = path.join(process.env.SystemRoot ?? "C:\\Windows", "System32");
    // 仅 System32：where 可用但 git 不在 PATH；固定路径 fallback 经 defaultGitExists 探测不存在
    const r = runBridge({
      home,
      panelId: "p-behavior-nobash",
      input: JSON.stringify({ context_window: { used_percentage: 42 } }),
      args: ['"~/.claude/statusline-deepseek.sh"'],
      pathOverride: system32,
    });
    expect(r.status).toBe(0);
    expect(r.stderr).toBe("");
    expect(r.stdout).toBe(PLACEHOLDER);
  });

  it("非 .sh 命令走系统 shell 透传", () => {
    const home = makeHome();
    const r = runBridge({
      home,
      panelId: "p-behavior-2",
      input: JSON.stringify({ context_window: { used_percentage: 42 } }),
      args: ["echo marker-cmdline-ok"],
    });
    expect(r.status).toBe(0);
    expect(r.stderr).toBe("");
    expect(r.stdout).toContain("marker-cmdline-ok");
  });

  it("透传失败 → stdout 占位文本（C10 保持：stderr 空 + exit 0）", () => {
    const home = makeHome();
    const r = runBridge({
      home,
      panelId: "p-behavior-3",
      input: JSON.stringify({ context_window: { used_percentage: 42 } }),
      args: ["definitely-not-a-real-cmd-xyz"],
    });
    expect(r.status).toBe(0);
    expect(r.stderr).toBe("");
    expect(r.stdout).toBe(PLACEHOLDER);
  });

  it("信号文件 payload：ContextUsage + usedPercentage + panelId + sessionId + cwd", () => {
    const home = makeHome();
    const r = runBridge({
      home,
      panelId: "p-behavior-4",
      input: JSON.stringify({
        context_window: { used_percentage: 42.5 },
        session_id: "s1",
        cwd: "c:/x",
      }),
    });
    expect(r.status).toBe(0);
    expect(r.stderr).toBe("");
    // 恰好 1 个信号文件
    const signals = listSignals(home);
    expect(signals.length).toBe(1);
    const payload = JSON.parse(
      fs.readFileSync(
        path.join(home, ".slterminal", "hooks-events", signals[0]),
        "utf8",
      ),
    );
    expect(payload.panelId).toBe("p-behavior-4");
    expect(payload.cliId).toBe("claude");
    expect(payload.event).toBe("ContextUsage");
    expect(payload.usedPercentage).toBe(42.5);
    expect(payload.sessionId).toBe("s1");
    expect(payload.cwd).toBe("c:/x");
  });

  it("节流：同取整值 1s 内不重复写；值变化即写", () => {
    const home = makeHome();
    const stdin = (pct: number) =>
      JSON.stringify({ context_window: { used_percentage: pct } });
    // 第一次 42.8 → round 43 写入
    runBridge({ home, panelId: "p-behavior-5", input: stdin(42.8) });
    // 第二次 43.2 → round 43 与上次相同且 <1s → 不写（信号数仍 1）
    runBridge({ home, panelId: "p-behavior-5", input: stdin(43.2) });
    expect(listSignals(home).length).toBe(1);
    // 值变化（55 → round 55）→ 写（信号数 2）
    runBridge({ home, panelId: "p-behavior-5", input: stdin(55) });
    expect(listSignals(home).length).toBe(2);
  });

  it("无 SLTERM_PANEL_ID → 静默退出不写信号（C3/C10）", () => {
    const home = makeHome();
    const r = runBridge({
      home,
      input: JSON.stringify({ context_window: { used_percentage: 42 } }),
    });
    expect(r.status).toBe(0);
    expect(r.stderr).toBe("");
    expect(listSignals(home).length).toBe(0);
  });

  it("非法 JSON stdin → 静默退出不写信号（C10）", () => {
    const home = makeHome();
    const r = runBridge({ home, panelId: "p-behavior-7", input: "not json" });
    expect(r.status).toBe(0);
    expect(r.stderr).toBe("");
    expect(listSignals(home).length).toBe(0);
  });

  it("空 stdin → 静默退出（C10）", () => {
    const home = makeHome();
    const r = runBridge({ home, panelId: "p-behavior-8", input: "" });
    expect(r.status).toBe(0);
    expect(r.stderr).toBe("");
    expect(listSignals(home).length).toBe(0);
  });
});
