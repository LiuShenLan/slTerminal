//! Git 方法族收口(04 篇改造节 5;子进程执行体归 07 篇 git 域)。
//!
//! 暴露面 = `git.status|diff|rollback|unstage` 四方法。校验形态照抄上游
//! 防陈旧三板斧:显式 pane + `expected_cwd`;服务端收到后重查 pane cwd 与
//! repo root;写操作必须带 `revision`(服务端 status 摘要),写前重跑 status
//! 不一致即 `git_stale`。路径参数是纯 argv 元素,永不解释为 shell 命令或
//! glob(执行侧统一 `--literal-pathspecs`)。
//!
//! M7.3 只落协议面:请求解析/校验/陈旧判定;07 篇子进程封装接线后由 UI
//! owner 执行,`--no-optional-locks` 纪律归 07。

use serde::Deserialize;
use serde_json::{Value, json};

use super::command::parse_params;
use super::{ApiError, RuntimeCommand};

/// 四方法操作名白名单(方法名 → operation,参数里不接受 operation 字段)。
pub(super) const OPERATIONS: &[&str] = &["status", "diff", "rollback", "unstage"];

/// `git.*` 请求。`operation` 由方法名派生(serde skip),客户端不可改写,
/// 避免「方法叫 status、参数要求 rollback」的歧义。
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub(crate) window_id: u64,
    pub(crate) pane_id: u64,
    /// 客户端看到的 pane cwd;服务端重查后必须一致。
    pub(crate) expected_cwd: String,
    #[serde(default)]
    pub(crate) path: String,
    /// 写操作必带:64 位 hex(status 摘要)。
    #[serde(default)]
    pub(crate) revision: String,
    #[serde(skip)]
    pub(crate) operation: String,
}

impl Request {
    pub(super) fn is_write(&self) -> bool {
        matches!(self.operation.as_str(), "rollback" | "unstage")
    }

    /// 写前状态摘要校验:与请求携带的 revision 不一致即拒绝。
    #[allow(dead_code)] // 消费点 = 07 篇 git 子进程执行器(M7.5/M8 接线)。
    pub(super) fn verify_revision(&self, current_revision: &str) -> Result<(), ApiError> {
        if self.is_write() && self.revision != current_revision {
            return Err(ApiError::new(
                "git_stale",
                "repository state changed; refresh before writing",
            )
            .details(json!({
                "revision": self.revision,
                "current_revision": current_revision
            })));
        }
        Ok(())
    }
}

impl Request {
    /// pane cwd 与 repo root 双重核对。设计稿要求两者都等于
    /// `expected_cwd`:Git 方法只对根 checkout 生效,子目录 pane 需先回到根。
    #[allow(dead_code)] // 消费点 = 07 篇 git 子进程执行器(M7.5/M8 接线)。
    pub(super) fn verify_repository(
        &self,
        pane_cwd: &str,
        repo_root: &str,
    ) -> Result<(), ApiError> {
        if pane_cwd != self.expected_cwd || repo_root != self.expected_cwd {
            return Err(ApiError::new(
                "git_target_changed",
                "the pane working directory or repository root changed; reopen Git",
            )
            .details(json!({
                "expected_cwd": self.expected_cwd,
                "pane_cwd": pane_cwd,
                "repo_root": repo_root
            })));
        }
        Ok(())
    }
}

pub(super) fn command_from_method(
    method: &str,
    params: &Value,
) -> Result<RuntimeCommand, ApiError> {
    let operation = method
        .strip_prefix("git.")
        .ok_or_else(|| ApiError::new("method_not_found", "unknown Git operation"))?;
    if !OPERATIONS.contains(&operation) {
        return Err(ApiError::new("method_not_found", "unknown Git operation"));
    }
    let mut request: Request = parse_params(params)?;
    request.operation = operation.to_owned();
    validate_request(&request)?;
    Ok(RuntimeCommand::Git {
        window_id: Some(request.window_id),
        pane_id: request.pane_id,
        request,
    })
}

fn validate_request(request: &Request) -> Result<(), ApiError> {
    if request.window_id == 0
        || request.pane_id == 0
        || request.expected_cwd.is_empty()
        || request.expected_cwd.len() > 4096
        || request.expected_cwd.contains('\0')
    {
        return Err(ApiError::invalid_params(
            "an explicit pane and working directory are required",
        ));
    }
    if request.path.len() > 4096 || request.path.contains('\0') {
        return Err(ApiError::invalid_params(
            "Git path exceeds its 4096-byte limit or contains NUL",
        ));
    }
    match request.operation.as_str() {
        // diff/rollback/unstage 都针对具体文件;status 只读全仓库。
        "diff" | "rollback" | "unstage" if request.path.trim().is_empty() => {
            return Err(ApiError::invalid_params(
                "this Git operation requires a non-empty path",
            ));
        }
        _ => {}
    }
    // revision 只接受空或 64 位 hex;写操作必须非空。
    let revision_valid = request.revision.is_empty()
        || (request.revision.len() == 64
            && request
                .revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()));
    if !revision_valid {
        return Err(ApiError::invalid_params(
            "revision must be a 64-hex Git status digest",
        ));
    }
    if request.is_write() && request.revision.is_empty() {
        return Err(ApiError::invalid_params(
            "a current Git status revision is required",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod git_tests {
    use super::*;

    fn revision() -> String {
        "0123456789abcdef".repeat(4)
    }

    fn params(operation: &str) -> Value {
        let mut value = json!({
            "window_id": 1,
            "pane_id": 2,
            "expected_cwd": "D:/repo",
        });
        if matches!(operation, "diff" | "rollback" | "unstage") {
            value["path"] = json!("src/main.rs");
        }
        if matches!(operation, "rollback" | "unstage") {
            value["revision"] = json!(revision());
        }
        value
    }

    fn command(operation: &str) -> Result<RuntimeCommand, ApiError> {
        command_from_method(&format!("git.{operation}"), &params(operation))
    }

    #[test]
    fn four_operations_are_accepted_and_trimmed_write_methods_are_not() {
        for operation in OPERATIONS {
            assert!(command(operation).is_ok(), "{operation}");
        }
        for method in [
            "git.history",
            "git.stage",
            "git.commit",
            "git.fetch",
            "git.pull",
            "git.push",
        ] {
            assert_eq!(
                command_from_method(method, &json!({}))
                    .expect_err("trimmed method")
                    .code,
                "method_not_found",
                "{method}"
            );
        }
    }

    #[test]
    fn operation_is_derived_from_the_method_not_from_params() {
        let mut with_operation = params("status");
        with_operation["operation"] = json!("rollback");
        assert_eq!(
            command_from_method("git.status", &with_operation)
                .expect_err("operation is not a client field")
                .code,
            "invalid_params"
        );
        let RuntimeCommand::Git { request, .. } = command("status").expect("status command") else {
            panic!("git method must route to RuntimeCommand::Git");
        };
        assert_eq!(request.operation, "status");
        assert!(!request.is_write());
    }

    #[test]
    fn required_fields_and_ranges_are_enforced() {
        for operation in OPERATIONS {
            let mut missing_window = params(operation);
            missing_window
                .as_object_mut()
                .expect("object")
                .remove("window_id");
            assert!(command_from_method(&format!("git.{operation}"), &missing_window).is_err());
            let mut extra = params(operation);
            extra["oracle"] = json!(true);
            assert!(command_from_method(&format!("git.{operation}"), &extra).is_err());
        }
        let mut zero_pane = params("status");
        zero_pane["pane_id"] = json!(0);
        assert!(command_from_method("git.status", &zero_pane).is_err());

        let mut long_cwd = params("status");
        long_cwd["expected_cwd"] = json!("x".repeat(4097));
        assert!(command_from_method("git.status", &long_cwd).is_err());

        let mut nul_cwd = params("status");
        nul_cwd["expected_cwd"] = json!("D:/repo\0other");
        assert!(command_from_method("git.status", &nul_cwd).is_err());

        let mut long_path = params("diff");
        long_path["path"] = json!("x".repeat(4097));
        assert!(command_from_method("git.diff", &long_path).is_err());

        let mut empty_path = params("diff");
        empty_path["path"] = json!("  ");
        assert!(command_from_method("git.diff", &empty_path).is_err());

        let mut missing_revision = params("rollback");
        missing_revision
            .as_object_mut()
            .expect("object")
            .remove("revision");
        assert!(command_from_method("git.rollback", &missing_revision).is_err());

        // 40 位短 hash 与非法字符都不收。
        let mut short_revision = params("rollback");
        short_revision["revision"] = json!("0".repeat(40));
        assert!(command_from_method("git.rollback", &short_revision).is_err());
        let mut bad_revision = params("unstage");
        bad_revision["revision"] = json!("z".repeat(64));
        assert!(command_from_method("git.unstage", &bad_revision).is_err());

        // 读操作不要求 revision,但给了就必须是合法形状。
        let mut malformed_read_revision = params("status");
        malformed_read_revision["revision"] = json!("not-a-digest");
        assert!(command_from_method("git.status", &malformed_read_revision).is_err());
    }

    #[test]
    fn paths_are_stored_verbatim_and_never_expanded() {
        let path = "docs/with space/*.md $(rm -rf /)";
        let mut value = params("diff");
        value["path"] = json!(path);
        let RuntimeCommand::Git { request, .. } =
            command_from_method("git.diff", &value).expect("diff command")
        else {
            panic!("git method must route to RuntimeCommand::Git");
        };
        assert_eq!(request.path, path);
        assert_eq!(request.operation, "diff");
    }

    #[test]
    fn stale_revision_and_cwd_drift_are_rejected() {
        let RuntimeCommand::Git { request, .. } = command("rollback").expect("rollback command")
        else {
            panic!("git method must route to RuntimeCommand::Git");
        };
        assert!(request.verify_revision(&revision()).is_ok());
        assert_eq!(
            request
                .verify_revision(&"f".repeat(64))
                .expect_err("stale revision")
                .code,
            "git_stale"
        );
        // 读操作不需要 revision,也不因 revision 不同而拒。
        let RuntimeCommand::Git { request, .. } = command("status").expect("status command") else {
            panic!("git method must route");
        };
        assert!(request.verify_revision("unrelated").is_ok());

        let RuntimeCommand::Git { request, .. } = command("status").expect("status command") else {
            panic!("git method must route");
        };
        assert!(request.verify_repository("D:/repo", "D:/repo").is_ok());
        assert_eq!(
            request
                .verify_repository("D:/repo/sub", "D:/repo")
                .expect_err("pane cwd drift")
                .code,
            "git_target_changed"
        );
        assert_eq!(
            request
                .verify_repository("D:/repo", "D:/other")
                .expect_err("repo root drift")
                .code,
            "git_target_changed"
        );
    }
}
