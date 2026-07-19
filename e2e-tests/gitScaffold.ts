/**
 * E2E git 仓库脚手架（Node 侧工具）。
 * 用例间 tempdir 隔离，execSync 调系统 git CLI（CI runner 有 git）。
 */

import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execSync } from "node:child_process";

/** makeGitRepo 的参数——描述仓库变更场景 */
export interface GitRepoScenario {
  /** 需要先提交再修改的文件名列表（提交后覆盖内容 → git status 显示为 modified） */
  modified?: string[];
  /** 提交后新建但不 add 的文件名列表（→ git status 显示为 untracked） */
  untracked?: string[];
}

/**
 * 在临时目录创建 git 仓库并应用指定场景。
 * 步骤：tempdir → git init → 写基线文件（含 modified 列表文件）→ commit →
 *        按 scenario 修改/新建文件 → 返回仓库路径。
 */
export function makeGitRepo(scenario: GitRepoScenario): string {
  const dir = mkdtempSync(join(tmpdir(), "slterm-e2e-git-"));

  // git init + 设置用户
  execSync("git init", { cwd: dir, stdio: "pipe" });
  execSync('git config user.email "e2e@test.com"', { cwd: dir, stdio: "pipe" });
  execSync('git config user.name "E2E Test"', { cwd: dir, stdio: "pipe" });

  // 基线文件：README.md 确保必有内容可提交 + modified 列表中的文件
  writeFileSync(join(dir, "README.md"), "# baseline\n", "utf8");
  if (scenario.modified) {
    for (const f of scenario.modified) {
      writeFileSync(join(dir, f), `initial content of ${f}\n`, "utf8");
    }
  }

  // 提交基线
  execSync("git add -A", { cwd: dir, stdio: "pipe" });
  execSync('git commit -m "initial commit"', { cwd: dir, stdio: "pipe" });

  // 场景：修改已跟踪文件 → modified
  if (scenario.modified) {
    for (const f of scenario.modified) {
      writeFileSync(join(dir, f), `modified: ${f}\n`, "utf8");
    }
  }

  // 场景：新建未跟踪文件 → untracked
  if (scenario.untracked) {
    for (const f of scenario.untracked) {
      writeFileSync(join(dir, f), `new untracked file: ${f}\n`, "utf8");
    }
  }

  return dir;
}

/** 递归删除 git 仓库临时目录。幂等（路径不存在不抛错）。 */
export function cleanupGitRepo(path: string): void {
  rmSync(path, { recursive: true, force: true });
}
