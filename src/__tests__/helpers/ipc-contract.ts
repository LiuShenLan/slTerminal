// helpers/ipc-contract.ts — IPC 契约测试共享工厂（IHE-06）
//
// 声明式 schema 驱动四维断言（命令名 / 参数结构 / 正常返回 / 异常传播），
// 供 ipc-contract / ipc-hooks-contract / ipc-hooks-config-contract /
// ipc-claude-history-contract 四文件复用。
//
// ⚠️ mockIPC 盲区（IHE-01）：mockIPC 只守 JS 侧形状——camelCase/snake_case
// 真实转换、Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在
// mock 层验证，真实序列化由 L4 E2E 守卫（详见 src/ipc/CLAUDE.md「mockIPC 盲区声明」）。

import { describe, it, expect, vi } from "vitest";
import { mockIPC } from "@tauri-apps/api/mocks";

/** 声明式 IPC 契约用例 schema（每条覆盖四维中的一维或多维） */
export interface IpcContractCase {
  /** 用例名（缺省自动生成 `${cmd} 合约`） */
  name?: string;
  /** 后端命令名（snake_case 逐字） */
  cmd: string;
  /** 调用 IPC wrapper 的闭包 */
  call: () => Promise<unknown>;
  /** mockIPC 正常返回值（undefined = 该命令无返回） */
  respond?: unknown;
  /** mockIPC 对该命令抛错（模拟后端 invoke 失败） */
  mockThrow?: string;
  /** 断言 invoke payload 精确匹配（toEqual） */
  expectArgs?: Record<string, unknown>;
  /** 断言 payload 键集合恰好为这些键（防单边字段漂移） */
  expectExactKeys?: string[];
  /** 断言 resolve 值（toEqual） */
  expectResult?: unknown;
  /** 断言 resolve 为 undefined（void 命令） */
  expectUndefined?: boolean;
  /** 断言 rejects（toThrow 消息/正则） */
  expectReject?: string | RegExp;
  /** 自定义 args 断言（Channel 实例、onmessage 绑定、字段存在性等） */
  assertArgs?: (args: Record<string, unknown>) => void;
}

/**
 * 以声明式用例表驱动一组 IPC 契约断言。
 *
 * 每个用例独立注册 mockIPC 并断言：
 * 1. 命令名逐字正确（snake_case）
 * 2. payload 结构（expectArgs 精确匹配 / expectExactKeys 键集合 / assertArgs 自定义）
 * 3. 正常返回透传（expectResult / expectUndefined）
 * 4. 异常传播（mockThrow + expectReject，或 mockThrow + expectResult 的 fallback 路径）
 */
export function describeIpcContract(
  scope: string,
  cases: IpcContractCase[],
): void {
  describe(scope, () => {
    for (const c of cases) {
      it(c.name ?? `${c.cmd} 合约`, async () => {
        const spy = vi.fn();
        mockIPC((cmd, args) => {
          spy(cmd, args);
          if (cmd === c.cmd && c.mockThrow !== undefined) {
            throw new Error(c.mockThrow);
          }
          return c.respond;
        });

        if (c.expectReject !== undefined) {
          await expect(c.call()).rejects.toThrow(c.expectReject);
        } else if (c.expectUndefined) {
          await expect(c.call()).resolves.toBeUndefined();
        } else {
          const result = await c.call();
          if (c.expectResult !== undefined) {
            expect(result).toEqual(c.expectResult);
          }
        }

        expect(spy).toHaveBeenCalledTimes(1);
        const [cmd, args] = spy.mock.calls[0] as [
          string,
          Record<string, unknown>,
        ];
        expect(cmd).toBe(c.cmd);
        if (c.expectArgs !== undefined) expect(args).toEqual(c.expectArgs);
        if (c.expectExactKeys !== undefined) {
          expect(Object.keys(args).sort()).toEqual([...c.expectExactKeys].sort());
        }
        if (c.assertArgs !== undefined) c.assertArgs(args);
      });
    }
  });
}
