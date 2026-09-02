// copyRelativePath.ts —「复制相对路径」共享动作（explorer 文件树 / workspace 页签两处右键菜单）
//
// 复制语义单点：目标位于项目根内 → 复制相对项目根的 Unix 格式（正斜杠）路径；
// 不在子树或未提供项目根 → 兜底复制完整绝对路径。成功无提示；失败仅 console.error（fire-and-forget）。

import { relativePath } from "./path";
import { writeText } from "../ipc/clipboard";

/** 复制相对路径（相对 projectRootPath）到剪贴板——语义见文件头注释 */
export function copyRelativePath(
  targetPath: string,
  projectRootPath?: string,
): void {
  // 先短路 !projectRootPath——relativePath 对 undefined root 会抛错，
  // 破坏 path.ts「空输入安全」消费契约
  const text =
    projectRootPath === undefined
      ? targetPath
      : (relativePath(targetPath, projectRootPath) ?? targetPath);
  writeText(text).catch((err) => console.error("复制相对路径失败:", err));
}
