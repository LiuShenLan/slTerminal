// slterm-statusline.js — slTerminal Claude Code statusline 桥接脚本
// 由 agent_hooks_inject（claude hooks provider）写入 ~/.slterminal/hooks/slterm-statusline.js
// 零依赖，仅使用 Node.js >= 18 内置 API
// 契约：任何代码路径 exit code 恒为 0，不向 stderr 输出（C10）
//
// 职责：
// 1. 读 stdin statusline JSON → 提取 context_window.used_percentage（官方口径）
// 2. 节流：取整后与上次相同不写 + 距上次写入 ≥1s（状态文件跨进程共享——每帧新 node 实例）
// 3. 写 context 用量信号文件（hooks-events 目录，tmp+rename 原子写，复用现有瞬态信号通道）
// 4. 包裹透传：argv[2] 为用户原 statusline 命令 → 执行并透传 stdin/stdout（失败静默降级）

const SCRIPT_VERSION = 4;

const fs = require("fs");
const path = require("path");
const os = require("os");
const { spawnSync } = require("child_process");

/** 节流最小间隔（毫秒）：写条件 = 取整值变化 AND 距上次 ≥1s */
const THROTTLE_MS = 1000;

(function () {
  try {
    let input = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", function (chunk) {
      input += chunk;
    });
    process.stdin.on("end", function () {
      try {
        // stdin 为空 → 静默退出（C10）
        if (!input.trim()) {
          process.exit(0);
        }

        var data = JSON.parse(input);
        var panelId = process.env.SLTERM_PANEL_ID;

        // 无页签标识（非 slTerminal 终端启动的 claude）→ 静默退出（C3/C10）
        if (!panelId) {
          process.exit(0);
        }

        // used_percentage 提取（数字校验；缺失/非法 → null 不写信号）
        var pct =
          data &&
          data.context_window &&
          typeof data.context_window.used_percentage === "number"
            ? data.context_window.used_percentage
            : null;

        var home = os.homedir();
        if (home) {
          // ── 节流：取整无变化不写 + 最小间隔（状态文件跨进程共享） ──
          var statePath = path.join(home, ".slterminal", "hooks", "statusline-state.json");
          var state = readState(statePath);
          var rounded = pct !== null ? Math.round(pct) : null;
          var now = Date.now();
          var shouldWrite =
            pct !== null &&
            (!state || state.lastPct !== rounded || now - state.lastAt >= THROTTLE_MS);

          if (shouldWrite) {
            var eventsDir = path.join(home, ".slterminal", "hooks-events");
            writeSignal(eventsDir, panelId, pct, data);
            writeState(statePath, rounded, now);
          }
        }

        // ── 包裹透传：执行用户原 statusline 命令（argv[2]），stdout 透传 ──
        // 执行失败静默降级（C10）——不写 stderr，状态行输出空
        var userCommand = process.argv[2];
        if (userCommand) {
          runUserCommand(userCommand, input);
        }

        process.exit(0);
      } catch (_) {
        // JSON 解析失败 / 目录不可写 / 写文件异常 → 静默退出（C10）
        process.exit(0);
      }
    });
    process.stdin.on("error", function () {
      process.exit(0);
    });
    process.stdin.resume();
  } catch (_) {
    // 顶层异常兜底（C10）
    process.exit(0);
  }
})();

/** 读取节流状态文件（缺失/损坏 → null 视为首次，C10） */
function readState(statePath) {
  try {
    var s = JSON.parse(fs.readFileSync(statePath, "utf8"));
    if (typeof s.lastPct === "number" && typeof s.lastAt === "number") {
      return s;
    }
    return null;
  } catch (_) {
    return null;
  }
}

/** 原子写节流状态文件（失败不影响主流程，C10） */
function writeState(statePath, lastPct, lastAt) {
  try {
    fs.mkdirSync(path.dirname(statePath), { recursive: true });
    var tmp = statePath + ".tmp";
    fs.writeFileSync(tmp, JSON.stringify({ lastPct: lastPct, lastAt: lastAt }), "utf8");
    fs.renameSync(tmp, statePath);
  } catch (_) {
    // 静默（C10）
  }
}

/** 写 context 用量信号文件（tmp+rename 原子写，payload 契约与 reporter 对齐） */
function writeSignal(dir, panelId, pct, data) {
  try {
    fs.mkdirSync(dir, { recursive: true });
    // payload 必填键与 AgentEventPayload DTO 对齐（panelId/event/timestamp/sessionId/cwd）；
    // usedPercentage 为可选扩展键（serde default，旧信号兼容）
    var payload = {
      panelId: panelId,
      cliId: "claude",
      event: "ContextUsage",
      timestamp: Date.now(),
      sessionId: (data && data.session_id) || "",
      cwd: (data && data.cwd) || "",
      usedPercentage: pct,
    };
    var safeId = panelId.replace(/[^a-zA-Z0-9_-]/g, "_");
    var rnd = Math.random().toString(36).slice(2, 8);
    var base = payload.timestamp + "_" + safeId + "_" + payload.event + "_" + rnd;
    var tmp = path.join(dir, base + ".tmp");
    var dst = path.join(dir, base + ".json");
    fs.writeFileSync(tmp, JSON.stringify(payload), "utf8");
    fs.renameSync(tmp, dst);
  } catch (_) {
    // 静默（C10）
  }
}

/** 执行用户原 statusline 命令并透传输出（失败静默降级，C10） */
function runUserCommand(cmd, input) {
  try {
    // ~ 展开（claude 配置允许 ~ 前缀；cmd/shell 不展开）
    var resolved = cmd.replace(/^~(?=[/\\])/, os.homedir());
    // .sh 脚本经 bash（Git Bash）执行；其余交系统 shell（.exe/.cmd 等）
    var isSh = /\.sh(\s|$)/.test(resolved);
    var r = isSh
      ? spawnSync("bash", ["-c", resolved], {
          input: input,
          encoding: "utf8",
          windowsHide: true,
        })
      : spawnSync(resolved, {
          shell: true,
          input: input,
          encoding: "utf8",
          windowsHide: true,
        });
    if (r && r.stdout) {
      process.stdout.write(r.stdout);
    }
  } catch (_) {
    // 静默（C10）
  }
}
