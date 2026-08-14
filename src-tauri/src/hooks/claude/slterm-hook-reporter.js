// slterm-hook-reporter.js — slTerminal Claude Code hook 信号上报脚本
// 由 agent_hooks_inject（claude hooks provider）写入 ~/.slterminal/hooks/slterm-hook-reporter.js
// 零依赖，仅使用 Node.js >= 18 内置 API
// 契约：任何代码路径 exit code 恒为 0，不向 stderr 输出（C10）

const SCRIPT_VERSION = 4;

const fs = require("fs");
const path = require("path");
const os = require("os");

(function () {
  try {
    let input = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", function (chunk) {
      input += chunk;
    });
    process.stdin.on("end", function () {
      try {
        // stdin 为空 → 静默退出
        if (!input.trim()) {
          process.exit(0);
        }

        var data = JSON.parse(input);
        var panelId = process.env.SLTERM_PANEL_ID;

        // 无页签标识（非 slTerminal 终端启动的 claude）→ 静默退出（C3/C10）
        if (!panelId) {
          process.exit(0);
        }

        var home = os.homedir();
        if (!home) {
          process.exit(0);
        }

        var dir = path.join(home, ".slterminal", "hooks-events");
        // 确保信号目录存在（recursive，首次创建）
        fs.mkdirSync(dir, { recursive: true });

        // 按跨边界契约组装信号 payload（8 字段 + 显式 cliId，camelCase；决策 7）
        var payload = {
          panelId: panelId,
          cliId: "claude",
          event: data.hook_event_name || "",
          timestamp: Date.now(),
          sessionId: data.session_id || "",
          usageSourcePath: data.transcript_path || null,
          cwd: data.cwd || "",
          toolName: data.tool_name || null,
          notificationType: data.notification_type || null,
        };

        // 生成唯一文件名：时间戳 + 安全 panelId + 事件名 + 随机后缀
        var safeId = panelId.replace(/[^a-zA-Z0-9_-]/g, "_");
        var rnd = Math.random().toString(36).slice(2, 8);
        var base =
          payload.timestamp + "_" + safeId + "_" + payload.event + "_" + rnd;

        // 原子写：先写 .tmp 再 renameSync 成 .json（C2 备选 A）
        var tmp = path.join(dir, base + ".tmp");
        var dst = path.join(dir, base + ".json");
        fs.writeFileSync(tmp, JSON.stringify(payload), "utf8");
        fs.renameSync(tmp, dst);

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
