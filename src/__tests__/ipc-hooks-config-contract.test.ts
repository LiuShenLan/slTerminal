// ipc-hooks-config-contract.test.ts — hooks 配置 IPC wrapper 合约测试（P3-FE-05）
//
// 使用 mockIPC 拦截真实的 invoke 调用，照 ipc-hooks-contract.test.ts 模式验证：
// 1. 命令名正确（snake_case：hooks_config_read / hooks_config_write）
// 2. 参数结构正确（camelCase：layer / hooks / projectPath，Tauri 自动转 snake_case）
// 3. 返回透传（read 返回 hooks 子树或 null）
// 4. 异常传播

import { describe, it, expect, afterEach, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";

// 覆盖 setup.ts 全局 mock（若未来新增）——导入原始 ../ipc/hooksConfig 模块以测试真实 IPC 合约
vi.mock("../ipc/hooksConfig", async (importOriginal) => {
  return importOriginal<typeof import("../ipc/hooksConfig")>();
});

import * as hooksConfig from "../ipc/hooksConfig";

afterEach(() => {
  clearMocks();
});

// ═══════════════════════════════════════════════════════════════════
// readHooksConfig — hooks_config_read
// ═══════════════════════════════════════════════════════════════════

describe("readHooksConfig 合约", () => {
  // 维度 1：命令名
  it("应调用 hooks_config_read 命令", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "hooks_config_read") return null;
    });

    await hooksConfig.readHooksConfig("user");

    expect(spy).toHaveBeenCalledTimes(1);
    const [cmd] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(cmd).toBe("hooks_config_read");
  });

  // 维度 2：参数结构——user 层无 projectPath
  it("user 层调用仅传 { layer }（无 projectPath）", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "hooks_config_read") return null;
    });

    await hooksConfig.readHooksConfig("user");

    expect(spy).toHaveBeenCalledTimes(1);
    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(args).toEqual({ layer: "user" });
    // 键集合精确匹配：无 projectPath 键（user 层不应携带）
    expect(Object.keys(args as Record<string, unknown>)).toEqual(["layer"]);
  });

  // 维度 2：参数结构——project 层带 projectPath
  it("project 层调用传 { layer, projectPath }", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "hooks_config_read") return null;
    });

    await hooksConfig.readHooksConfig("project", "D:/repo");

    expect(spy).toHaveBeenCalledTimes(1);
    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(args).toEqual({ layer: "project", projectPath: "D:/repo" });
    expect(Object.keys(args as Record<string, unknown>)).toEqual([
      "layer",
      "projectPath",
    ]);
  });

  // 维度 3：正常返回透传——hooks 子树对象
  it("有 hooks 子树时透传原始 JSON 对象", async () => {
    const mockSubtree = {
      PreToolUse: [
        {
          matcher: "Bash",
          hooks: [
            { type: "command", command: "echo hi", timeout: 5 },
          ],
        },
      ],
    };
    mockIPC((cmd) => {
      if (cmd === "hooks_config_read") return { ...mockSubtree };
    });

    const result = await hooksConfig.readHooksConfig("user");

    expect(result).toEqual(mockSubtree);
  });

  // 维度 3：正常返回透传——null（文件不存在或无 hooks 键）
  it("文件不存在或无 hooks 键时透传 null", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_config_read") return null;
    });

    const result = await hooksConfig.readHooksConfig("user");

    expect(result).toBeNull();
  });

  // 维度 4：异常传播
  it("invoke 失败（JSON 损坏等）时异常应传播给调用方", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_config_read") throw new Error("配置文件损坏，请先修复");
    });

    await expect(hooksConfig.readHooksConfig("user")).rejects.toThrow(
      "配置文件损坏，请先修复",
    );
  });
});

// ═══════════════════════════════════════════════════════════════════
// writeHooksConfig — hooks_config_write
// ═══════════════════════════════════════════════════════════════════

describe("writeHooksConfig 合约", () => {
  // 维度 1：命令名
  it("应调用 hooks_config_write 命令", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "hooks_config_write") return;
    });

    await hooksConfig.writeHooksConfig("user", {});

    expect(spy).toHaveBeenCalledTimes(1);
    const [cmd] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(cmd).toBe("hooks_config_write");
  });

  // 维度 2：payload 键集合精确匹配——带 projectPath（local 层）
  it("local 层调用 payload 键集合精确为 { layer, hooks, projectPath }", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "hooks_config_write") return;
    });

    const hooks = {
      PreToolUse: [
        { matcher: "Bash", hooks: [{ type: "command", command: "echo hi" }] },
      ],
    };
    await hooksConfig.writeHooksConfig("local", hooks, "D:/repo");

    expect(spy).toHaveBeenCalledTimes(1);
    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(Object.keys(args as Record<string, unknown>).sort()).toEqual([
      "hooks",
      "layer",
      "projectPath",
    ]);
    expect(args.layer).toBe("local");
    expect(args.hooks).toEqual(hooks);
    expect(args.projectPath).toBe("D:/repo");
  });

  // 维度 2：payload 键集合精确匹配——无 projectPath（user 层）
  it("user 层调用 payload 键集合精确为 { layer, hooks }（无 projectPath）", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "hooks_config_write") return;
    });

    await hooksConfig.writeHooksConfig("user", { SessionStart: [] });

    expect(spy).toHaveBeenCalledTimes(1);
    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(Object.keys(args as Record<string, unknown>).sort()).toEqual([
      "hooks",
      "layer",
    ]);
    expect(args.layer).toBe("user");
  });

  // 维度 2：字段名是 hooks（非 content）
  it("payload 字段名应为 hooks（非 content）", async () => {
    const spy = vi.fn();
    mockIPC((cmd, args) => {
      spy(cmd, args);
      if (cmd === "hooks_config_write") return;
    });

    await hooksConfig.writeHooksConfig("user", {});

    const [, args] = spy.mock.calls[0] as [string, Record<string, unknown>];
    expect(args).toHaveProperty("hooks");
    expect(args).not.toHaveProperty("content");
  });

  // 维度 3：正常返回 void
  it("正常返回 void", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_config_write") return;
    });

    const result = await hooksConfig.writeHooksConfig("user", {});
    expect(result).toBeUndefined();
  });

  // 维度 4：异常传播
  it("invoke 失败（原文件损坏拒绝写入等）时异常应传播", async () => {
    mockIPC((cmd) => {
      if (cmd === "hooks_config_write") throw new Error("原文件 JSON 损坏，拒绝写入");
    });

    await expect(hooksConfig.writeHooksConfig("user", {})).rejects.toThrow(
      "原文件 JSON 损坏，拒绝写入",
    );
  });
});
