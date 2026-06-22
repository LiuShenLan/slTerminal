import { describe, it, expect, beforeEach } from "vitest";
import { useSessions } from "../stores/sessions";
import type { SessionInfo } from "../stores/sessions";

/** 构造测试用 SessionInfo */
function makeSession(overrides?: Partial<SessionInfo>): SessionInfo {
  return {
    sessionId: "session-1",
    panelId: "panel-1",
    cwd: "/tmp/test",
    isActive: true,
    ...overrides,
  };
}

describe("sessions store", () => {
  beforeEach(() => {
    useSessions.setState({ sessions: {} });
  });

  // 1. 初始状态 sessions 为空对象
  it("初始状态 sessions 为空对象", () => {
    const { sessions } = useSessions.getState();
    expect(sessions).toEqual({});
  });

  // 2. setSession 添加新 session
  it("setSession 添加新 session", () => {
    const info = makeSession();
    useSessions.getState().setSession(info.panelId, info);

    const { sessions } = useSessions.getState();
    expect(Object.keys(sessions)).toHaveLength(1);
    expect(sessions[info.panelId]).toEqual(info);
  });

  // 3. setSession 更新已有 session（幂等）
  it("setSession 更新已有 session（幂等）", () => {
    const info = makeSession();
    useSessions.getState().setSession(info.panelId, info);

    const updated = makeSession({ cwd: "/home/updated", isActive: false });
    useSessions.getState().setSession(info.panelId, updated);

    const { sessions } = useSessions.getState();
    expect(Object.keys(sessions)).toHaveLength(1);
    expect(sessions[info.panelId]).toEqual(updated);
    expect(sessions[info.panelId].sessionId).toBe("session-1");
  });

  // 4. setActive 设置 isActive=true
  it("setActive 设置 isActive=true", () => {
    const info = makeSession({ isActive: false });
    useSessions.getState().setSession(info.panelId, info);

    useSessions.getState().setActive(info.panelId, true);

    const { sessions } = useSessions.getState();
    expect(sessions[info.panelId].isActive).toBe(true);
  });

  // 5. setActive 设置 isActive=false
  it("setActive 设置 isActive=false", () => {
    const info = makeSession({ isActive: true });
    useSessions.getState().setSession(info.panelId, info);

    useSessions.getState().setActive(info.panelId, false);

    const { sessions } = useSessions.getState();
    expect(sessions[info.panelId].isActive).toBe(false);
  });

  // 6. setActive 对不存在的 panelId — 静默忽略（不崩溃，state 不变）
  it("setActive 对不存在的 panelId 静默忽略", () => {
    // 先存一个 session 作为参照
    const info = makeSession({ panelId: "existing", isActive: false });
    useSessions.getState().setSession(info.panelId, info);

    const beforeState = useSessions.getState().sessions;
    // 对不存在的 panelId 调 setActive
    useSessions.getState().setActive("nonexistent", true);

    const { sessions } = useSessions.getState();
    // state 应完全不变
    expect(sessions).toEqual(beforeState);
  });

  // 7. removeSession 删除存在的 session
  it("removeSession 删除存在的 session", () => {
    const info = makeSession();
    useSessions.getState().setSession(info.panelId, info);

    useSessions.getState().removeSession(info.panelId);

    const { sessions } = useSessions.getState();
    expect(Object.keys(sessions)).toHaveLength(0);
    expect(sessions[info.panelId]).toBeUndefined();
  });

  // 8. removeSession 删除不存在的 session — 静默忽略
  it("removeSession 删除不存在的 session 静默忽略", () => {
    const info = makeSession();
    useSessions.getState().setSession(info.panelId, info);

    const beforeState = useSessions.getState().sessions;
    useSessions.getState().removeSession("nonexistent");

    const { sessions } = useSessions.getState();
    expect(sessions).toEqual(beforeState);
  });

  // 9. 多个 session 共存场景
  it("多个 session 共存场景", () => {
    const s1 = makeSession({ sessionId: "s1", panelId: "panel-1", cwd: "/a" });
    const s2 = makeSession({ sessionId: "s2", panelId: "panel-2", cwd: "/b", isActive: false });
    const s3 = makeSession({ sessionId: "s3", panelId: "panel-3", cwd: "/c" });

    useSessions.getState().setSession(s1.panelId, s1);
    useSessions.getState().setSession(s2.panelId, s2);
    useSessions.getState().setSession(s3.panelId, s3);

    let { sessions } = useSessions.getState();
    expect(Object.keys(sessions)).toHaveLength(3);
    expect(sessions["panel-1"]).toEqual(s1);
    expect(sessions["panel-2"]).toEqual(s2);
    expect(sessions["panel-3"]).toEqual(s3);

    // 删除中间一个，验证其余不受影响
    useSessions.getState().removeSession("panel-2");
    sessions = useSessions.getState().sessions;
    expect(Object.keys(sessions)).toHaveLength(2);
    expect(sessions["panel-1"]).toEqual(s1);
    expect(sessions["panel-3"]).toEqual(s3);

    // 切换 isActive 不影响其他 session
    useSessions.getState().setActive("panel-3", false);
    sessions = useSessions.getState().sessions;
    expect(sessions["panel-3"].isActive).toBe(false);
    expect(sessions["panel-1"].isActive).toBe(true);
  });

  // 10. SessionInfo 完整字段验证（sessionId, panelId, cwd, isActive）
  it("SessionInfo 完整字段验证", () => {
    const info: SessionInfo = {
      sessionId: "full-session",
      panelId: "full-panel",
      cwd: "D:\\projects\\demo",
      isActive: true,
    };
    useSessions.getState().setSession(info.panelId, info);

    const stored = useSessions.getState().sessions[info.panelId];
    expect(stored.sessionId).toBe("full-session");
    expect(stored.panelId).toBe("full-panel");
    expect(stored.cwd).toBe("D:\\projects\\demo");
    expect(stored.isActive).toBe(true);

    // cwd 为可选字段，允许 undefined
    const noCwd = makeSession({
      sessionId: "no-cwd",
      panelId: "panel-no-cwd",
      cwd: undefined,
    });
    useSessions.getState().setSession(noCwd.panelId, noCwd);
    expect(
      useSessions.getState().sessions["panel-no-cwd"].cwd,
    ).toBeUndefined();
  });
});
