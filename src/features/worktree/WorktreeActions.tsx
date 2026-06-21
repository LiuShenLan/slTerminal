// WorktreeActions — 工作树操作对话框组件
//
// CreateWorktreeDialog: 输入分支名 → 净化 → git addWorktree → 自动创建默认操作页面。
// DeleteWorktreeConfirm: 列出受影响面板 → checkbox gate → 确认 → removeWorktree → 清理 store。

import React, { useState, useCallback } from "react";
import type { Project, OperationPage } from "../../stores/projects";
import type { WorktreeInfo, WorktreeBinding } from "../../types/git";
import { git } from "../../ipc";
import { createPageId } from "../../stores/projects";

// ---- 通用样式 ----

const overlayStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(0,0,0,0.6)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  zIndex: 2000,
};

const dialogStyle: React.CSSProperties = {
  background: "#252526",
  border: "1px solid #454545",
  borderRadius: 8,
  padding: 20,
  minWidth: 360,
  maxWidth: 480,
  color: "#D4D4D4",
  fontSize: 13,
  boxShadow: "0 8px 24px rgba(0,0,0,0.5)",
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  background: "#1E1E1E",
  border: "1px solid #454545",
  borderRadius: 4,
  color: "#D4D4D4",
  padding: "6px 8px",
  fontSize: 13,
  outline: "none",
  marginTop: 8,
};

const btnRowStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "flex-end",
  gap: 8,
  marginTop: 16,
};

const primaryBtnStyle: React.CSSProperties = {
  background: "#0E639C",
  border: "none",
  color: "#fff",
  padding: "6px 14px",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: 13,
};

const secondaryBtnStyle: React.CSSProperties = {
  background: "#3C3C3C",
  border: "none",
  color: "#D4D4D4",
  padding: "6px 14px",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: 13,
};

/** 净化分支名（与 Git 要求一致） */
export function sanitizeBranchName(name: string): string {
  return name
    .trim()
    .replace(/[\s\\~^:?*[\]]/g, "-")
    .replace(/@{/g, "-")
    .replace(/\.\./g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 200);
}

// ---- CreateWorktreeDialog ----

interface CreateWorktreeDialogProps {
  /** 所属项目 */
  project: Project;
  /** 关闭回调 */
  onClose: () => void;
  /** 创建成功回调：返回新 worktree + 默认页面 */
  onCreated: (projectId: string, worktree: WorktreeInfo, defaultPage: OperationPage) => void;
}

/** 新建 worktree 对话框 */
export const CreateWorktreeDialog: React.FC<CreateWorktreeDialogProps> = ({
  project,
  onClose,
  onCreated,
}) => {
  const [branchName, setBranchName] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = useCallback(async () => {
    const sanitized = sanitizeBranchName(branchName);
    if (!sanitized) {
      setError("分支名不能为空");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const worktree = await git.addWorktree(project.rootPath, sanitized);

      // 构造默认操作页面（绑定到新 worktree）
      const binding: WorktreeBinding = {
        worktreePath: worktree.path,
        branchName: worktree.branch,
      };
      const defaultPage: OperationPage = {
        pageId: createPageId(),
        name: sanitized,
        layout: {},
        binding,
        createdAt: Date.now(),
        lastAccessedAt: Date.now(),
      };

      onCreated(project.projectId, worktree, defaultPage);
    } catch (err) {
      setError(`创建失败: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  }, [branchName, project, onCreated]);

  return (
    <div style={overlayStyle} onClick={onClose}>
      <div style={dialogStyle} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ margin: "0 0 8px", fontSize: 15 }}>
          新建工作树 — {project.name}
        </h3>
        <label style={{ color: "#999", fontSize: 12 }}>
          分支名
        </label>
        <input
          style={inputStyle}
          placeholder="feature/my-branch"
          value={branchName}
          onChange={(e) => setBranchName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleCreate();
            if (e.key === "Escape") onClose();
          }}
          autoFocus
          disabled={loading}
        />
        {error && (
          <div style={{ color: "#F44747", marginTop: 8, fontSize: 12 }}>
            {error}
          </div>
        )}
        <div style={btnRowStyle}>
          <button style={secondaryBtnStyle} onClick={onClose} disabled={loading}>
            取消
          </button>
          <button style={primaryBtnStyle} onClick={handleCreate} disabled={loading}>
            {loading ? "创建中..." : "创建"}
          </button>
        </div>
      </div>
    </div>
  );
};

// ---- DeleteWorktreeConfirm ----

interface DeleteWorktreeConfirmProps {
  /** worktree 信息 */
  worktree: WorktreeInfo;
  /** 受影响的操作页面名称列表 */
  affectedPages: string[];
  /** 关闭回调 */
  onClose: () => void;
  /** 确认删除回调 */
  onConfirm: () => void;
}

/** 删除 worktree 确认对话框 */
export const DeleteWorktreeConfirm: React.FC<DeleteWorktreeConfirmProps> = ({
  worktree,
  affectedPages,
  onClose,
  onConfirm,
}) => {
  const [checked, setChecked] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleConfirm = useCallback(async () => {
    if (!checked) return;
    setLoading(true);
    try {
      onConfirm();
    } finally {
      setLoading(false);
    }
  }, [checked, onConfirm]);

  return (
    <div style={overlayStyle} onClick={onClose}>
      <div style={dialogStyle} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ margin: "0 0 8px", fontSize: 15 }}>
          删除工作树
        </h3>
        <p style={{ color: "#D4D4D4", marginTop: 8 }}>
          分支: <span style={{ color: "#CE9178" }}>{worktree.branch}</span>
        </p>
        <p style={{ color: "#999", fontSize: 12, marginTop: 4 }}>
          路径: {worktree.path}
        </p>

        {affectedPages.length > 0 && (
          <div style={{ marginTop: 12 }}>
            <div style={{ color: "#F44747", fontSize: 12, marginBottom: 4 }}>
              以下操作页面将被删除:
            </div>
            <ul
              style={{
                margin: 0,
                paddingLeft: 20,
                color: "#999",
                fontSize: 12,
              }}
            >
              {affectedPages.map((name, i) => (
                <li key={i}>{name}</li>
              ))}
            </ul>
          </div>
        )}

        <label
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            marginTop: 16,
            cursor: "pointer",
            userSelect: "none",
            fontSize: 13,
          }}
        >
          <input
            type="checkbox"
            checked={checked}
            onChange={(e) => setChecked(e.target.checked)}
          />
          我确认要删除此工作树及其所有关联数据
        </label>

        <div style={btnRowStyle}>
          <button style={secondaryBtnStyle} onClick={onClose} disabled={loading}>
            取消
          </button>
          <button
            style={{
              ...primaryBtnStyle,
              background: checked ? "#D32F2F" : "#555",
              cursor: checked && !loading ? "pointer" : "not-allowed",
            }}
            onClick={handleConfirm}
            disabled={!checked || loading}
          >
            {loading ? "删除中..." : "确认删除"}
          </button>
        </div>
      </div>
    </div>
  );
};
