/// 应用数据目录解析模块 — 便携分发语义：应用数据存 exe 同级目录
///
/// BE-16 从 settings.rs 上提：settings/projects 两个持久化模块共享的
/// 目录解析 + 测试注入守卫 + 加载结果 DTO + 保存大小上限（消约束 #2 的跨模块穿透）。
///
/// - `resolve_app_data_dir`：纯函数（注入可失败点供错误分支测试）
/// - `app_data_dir`：当前 exe 目录（测试可经 [`AppDataDirGuard`] 覆盖）
/// - `AppDataDirGuard`：测试注入 RAII 守卫（settings/projects 命令层测试复用）
/// - `LoadResult<T>`：load 命令统一返回结构 `{ data, corrupted }`（BE-14/D11）
/// - `MAX_PERSIST_BYTES`：save 侧大小上限（SEC-11，settings/projects 共用）
use crate::error::AppError;
use std::path::PathBuf;

/// 持久化文件大小上限（SEC-11）：1MB，超限拒绝保存
pub(crate) const MAX_PERSIST_BYTES: usize = 1024 * 1024;

/// 数据目录环境变量覆盖键（BE-01）：E2E 测试/日常数据隔离经此显式指定
const DATA_DIR_ENV: &str = "SLTERM_DATA_DIR";

/// 加载结果 DTO（BE-14/D11）：`{ data, corrupted }`——前端据此区分
/// 「无数据」（corrupted=false）与「配置损坏已回退默认值」（corrupted=true，
/// 含 .bak 兜底命中——数据来自备份同样视为损坏态）。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadResult<T> {
    pub(crate) data: T,
    pub(crate) corrupted: bool,
}

/// 从可执行文件路径解析应用数据目录（纯函数，注入可失败点供 SPE-04 两错误分支测试）
pub(crate) fn resolve_app_data_dir(
    exe: Result<PathBuf, std::io::Error>,
) -> Result<PathBuf, AppError> {
    let exe = exe.map_err(|e| {
        tracing::warn!(error = %e, "无法获取可执行文件路径");
        AppError::IoKind {
            kind: "exe_dir".into(),
            message: "无法获取可执行文件路径".into(),
        }
    })?;
    let exe_dir = exe.parent().ok_or_else(|| AppError::IoKind {
        kind: "exe_dir".into(),
        message: "无法获取可执行文件所在目录".into(),
    })?;
    Ok(exe_dir.to_path_buf())
}

/// 测试用：app_data_dir() 覆盖注入槽（仅测试编译，生产零行为变更）
#[cfg(test)]
static APP_DATA_DIR_OVERRIDE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// 测试用 RAII 守卫：把 app_data_dir() 指向指定目录，Drop 时恢复原值
/// （防测试 panic 残留覆盖污染后续用例；settings/projects 模块命令层测试复用）
#[cfg(test)]
pub(crate) struct AppDataDirGuard(Option<PathBuf>);

#[cfg(test)]
impl AppDataDirGuard {
    pub(crate) fn set(dir: &std::path::Path) -> Self {
        let mut slot = APP_DATA_DIR_OVERRIDE.lock().unwrap();
        let prev = slot.clone();
        *slot = Some(dir.to_path_buf());
        AppDataDirGuard(prev)
    }
}

#[cfg(test)]
impl Drop for AppDataDirGuard {
    fn drop(&mut self) {
        *APP_DATA_DIR_OVERRIDE.lock().unwrap() = self.0.clone();
    }
}

/// 获取应用数据目录（exe 同级目录，适配便携分发）
pub(crate) fn app_data_dir() -> Result<PathBuf, AppError> {
    // 测试注入覆盖（仅测试编译，生产恒走 current_exe 路径）
    #[cfg(test)]
    {
        if let Some(dir) = APP_DATA_DIR_OVERRIDE.lock().unwrap().clone() {
            return Ok(dir);
        }
    }
    // E2E 隔离：环境变量显式指定数据目录（空串视为未设置）
    // 优先级语义：测试 guard（cfg(test)，生产零编译）> 环境变量 > exe 同级推导
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    resolve_app_data_dir(std::env::current_exe())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SPE-04: app_data_dir() 错误分支（路径解析纯函数注入可失败点） ──
    // （BE-16 随 app_data_dir 从 settings.rs 上提，测试归属 app_dir 模块）

    /// current_exe 失败 → IoKind("exe_dir")，消息含"无法获取可执行文件路径"
    #[test]
    fn resolve_app_data_dir_current_exe_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "模拟 current_exe 失败");
        let result = resolve_app_data_dir(Err(io_err));
        match result {
            Err(AppError::IoKind { kind, message }) => {
                assert_eq!(kind, "exe_dir");
                assert!(
                    message.contains("无法获取可执行文件路径"),
                    "消息应含错误提示，实际: {message}"
                );
            }
            other => panic!("current_exe 失败应映射为 IoKind，实际: {other:?}"),
        }
    }

    /// exe 无父目录（根路径）→ IoKind("exe_dir")，消息含"无法获取可执行文件所在目录"
    #[test]
    fn resolve_app_data_dir_exe_no_parent() {
        // 根路径的 parent() 为 None（Windows "C:\\" / Unix "/" 均无父目录）
        let root_path = if cfg!(windows) {
            std::path::PathBuf::from("C:\\")
        } else {
            std::path::PathBuf::from("/")
        };
        let result = resolve_app_data_dir(Ok(root_path));
        match result {
            Err(AppError::IoKind { kind, message }) => {
                assert_eq!(kind, "exe_dir");
                assert!(
                    message.contains("无法获取可执行文件所在目录"),
                    "消息应含错误提示，实际: {message}"
                );
            }
            other => panic!("exe 无父目录应映射为 IoKind，实际: {other:?}"),
        }
    }

    /// 正常路径：exe 位于目录下 → 返回该目录
    #[test]
    fn resolve_app_data_dir_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_app_data_dir(Ok(dir.path().join("slterminal.exe"))).unwrap();
        assert_eq!(result, dir.path(), "应返回 exe 所在目录");
    }

    /// LoadResult 序列化形态 = { data, corrupted }（serde camelCase，BE-14 契约）
    #[test]
    fn load_result_serializes_as_data_corrupted() {
        let r = LoadResult {
            data: serde_json::json!({ "fontSize": 14 }),
            corrupted: true,
        };
        let json = serde_json::to_string(&r).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["data"]["fontSize"], 14, "data 字段应携带数据");
        assert_eq!(v["corrupted"], true, "corrupted 字段应序列化");
        // settings 无文件场景：data 为 null
        let r2 = LoadResult::<Option<serde_json::Value>> {
            data: None,
            corrupted: false,
        };
        let json2 = serde_json::to_string(&r2).unwrap();
        assert!(
            json2.contains("\"data\":null"),
            "data 为 None 应序列化为 JSON null，实际: {json2}"
        );
    }

    /// 测试守卫生效：覆盖 app_data_dir 返回注入目录（命令层测试的注入机制）
    #[test]
    fn app_data_dir_honors_guard_override() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = AppDataDirGuard::set(dir.path());
        assert_eq!(
            app_data_dir().unwrap(),
            dir.path(),
            "守卫应覆盖 app_data_dir 返回值"
        );
    }

    // ── BE-01: SLTERM_DATA_DIR 环境变量覆盖（E2E/日常数据隔离） ──
    // 优先级语义：测试 guard > 环境变量 > exe 同级推导；每例结束 remove_var
    // 清理（L1 强制 --test-threads=1 无跨用例竞态）。

    /// ① SLTERM_DATA_DIR 生效：返回环境变量显式指定的目录
    #[test]
    fn app_data_dir_honors_env_override() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var(DATA_DIR_ENV, dir.path());
        let result = app_data_dir();
        std::env::remove_var(DATA_DIR_ENV);
        assert_eq!(result.unwrap(), dir.path(), "环境变量指定目录应生效");
    }

    /// ② 空串视为未设置：忽略并回落 exe 同级推导
    #[test]
    fn app_data_dir_ignores_empty_env() {
        std::env::set_var(DATA_DIR_ENV, "");
        let result = app_data_dir();
        std::env::remove_var(DATA_DIR_ENV);
        let exe_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert_eq!(result.unwrap(), exe_dir, "空串应忽略并回落 exe 同级推导");
    }

    /// ③ 测试 guard 优先于 env：guard 与 env 同设时返回 guard 值
    #[test]
    fn app_data_dir_guard_beats_env() {
        let guard_dir = tempfile::tempdir().unwrap();
        let env_dir = tempfile::tempdir().unwrap();
        std::env::set_var(DATA_DIR_ENV, env_dir.path());
        let _guard = AppDataDirGuard::set(guard_dir.path());
        let result = app_data_dir();
        std::env::remove_var(DATA_DIR_ENV);
        assert_eq!(
            result.unwrap(),
            guard_dir.path(),
            "测试 guard 应优先于环境变量"
        );
    }

    // ── 依赖真实 current_exe 的测试（SPE-06 ②） ──
    // 说明：以下两测试依赖真实 std::env::current_exe()（测试进程的可执行文件路径），
    // 未注入覆盖——验证的是生产 app_data_dir() 的便携分发语义（exe 同级目录）。
    // 测试进程必在运行，current_exe() 恒成功，故仅断言父目录关系与路径拼接。

    /// T2.1: app_data_dir 返回 current_exe 的父目录
    #[test]
    fn app_data_dir_returns_exe_parent() {
        let app_dir = app_data_dir().expect("app_data_dir 不应失败");
        let exe = std::env::current_exe().expect("current_exe 不应失败");
        let exe_dir = exe.parent().expect("exe 应有父目录");
        assert_eq!(app_dir, exe_dir, "app_data_dir 应返回 exe 所在目录");
    }

    /// T2.2: app_data_dir + settings.json 路径拼接
    #[test]
    fn app_data_dir_joins_settings_path() {
        let app_dir = app_data_dir().expect("app_data_dir 不应失败");
        let settings_path = app_dir.join("settings.json");
        assert!(
            settings_path.ends_with("settings.json"),
            "应指向 settings.json"
        );
        // 验证父目录存在（测试运行时 exe 目录必然存在）
        assert!(app_dir.exists(), "app_data_dir 返回的目录应存在");
    }
}
