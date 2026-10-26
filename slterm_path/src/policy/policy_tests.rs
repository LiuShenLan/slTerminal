use super::*;

fn absolute(path: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(format!("C:/{path}"))
    } else {
        PathBuf::from(format!("/{path}"))
    }
}

fn canonical(path: &str) -> CanonicalPath {
    CanonicalPath::new(absolute(path)).unwrap()
}

fn resolved(path: &str) -> ResolvedPath {
    ResolvedPath::new(
        canonical(path),
        Some(FileIdentity {
            volume: 7,
            file: 99,
        }),
    )
}

fn request(operation: PathOperation, lexical_path: PathBuf) -> PathRequest {
    PathRequest {
        operation,
        lexical_path,
        root_hints: Vec::new(),
        session_grant: None,
        expected_authorization_generation: 7,
    }
}

fn authorize(
    operation: PathOperation,
    target: &ResolvedPath,
    grants: &[PathGrant],
) -> Result<PathAuthorization, PathDenial> {
    PathPolicy::authorize(&request(operation, absolute("request")), target, grants, 7)
}

fn authorize_session(
    operation: PathOperation,
    target: &ResolvedPath,
    grant: PathGrant,
) -> Result<PathAuthorization, PathDenial> {
    let mut request = request(operation, absolute("request"));
    request.session_grant = Some(grant);
    PathPolicy::authorize(&request, target, &[], 7)
}

/// 能力位按集合组合，不是一个授权布尔值。
#[test]
fn capabilities_support_set_algebra() {
    let read_write = Capabilities::READ.union(Capabilities::WRITE);
    assert!(read_write.contains(Capabilities::READ));
    assert!(read_write.contains(Capabilities::WRITE));
    assert!(!read_write.contains(Capabilities::TERMINAL_CWD));
    assert_eq!(
        read_write.intersection(Capabilities::WRITE),
        Capabilities::WRITE
    );
    assert!(Capabilities::empty().is_empty());
    assert!(Capabilities::empty().contains(Capabilities::empty()));
}

/// 持久根只获得各自声明的最小能力。
#[test]
fn persistent_roots_have_minimal_default_capabilities() {
    let target = resolved("root/file");
    let repo = PathGrant::repo_root(canonical("root"));
    assert_eq!(repo.path(), &canonical("root"));
    assert_eq!(repo.capabilities(), Capabilities::READ);
    assert_eq!(repo.persistence(), GrantPersistence::Persistent);
    assert!(authorize(PathOperation::Read, &target, std::slice::from_ref(&repo)).is_ok());
    assert_eq!(
        authorize(PathOperation::Write, &target, std::slice::from_ref(&repo)),
        Err(PathDenial::Unauthorized(PathOperation::Write))
    );
    assert_eq!(
        authorize(PathOperation::TerminalCwd, &target, &[repo]),
        Err(PathDenial::Unauthorized(PathOperation::TerminalCwd))
    );

    let agent = PathGrant::agent_root(canonical("root"), false, false);
    assert!(authorize(PathOperation::Read, &target, std::slice::from_ref(&agent)).is_ok());
    assert_eq!(
        authorize(PathOperation::Write, &target, std::slice::from_ref(&agent)),
        Err(PathDenial::Unauthorized(PathOperation::Write))
    );
    assert_eq!(
        authorize(PathOperation::TerminalCwd, &target, &[agent]),
        Err(PathDenial::Unauthorized(PathOperation::TerminalCwd))
    );

    let writable = PathGrant::agent_root(canonical("root"), true, false);
    assert!(
        authorize(
            PathOperation::Write,
            &target,
            std::slice::from_ref(&writable)
        )
        .is_ok()
    );
    assert_eq!(
        authorize(PathOperation::TerminalCwd, &target, &[writable]),
        Err(PathDenial::Unauthorized(PathOperation::TerminalCwd))
    );
    let terminal = PathGrant::agent_root(canonical("root"), false, true);
    assert!(
        authorize(
            PathOperation::TerminalCwd,
            &target,
            std::slice::from_ref(&terminal)
        )
        .is_ok()
    );
    assert_eq!(
        authorize(PathOperation::Write, &target, &[terminal]),
        Err(PathDenial::Unauthorized(PathOperation::Write))
    );
}

/// 只读嵌套根不否决另一根的写能力，删除写根立即失效。
#[test]
fn nested_roots_union_capabilities_without_read_only_veto() {
    let target = resolved("root/nested/file");
    let read_only = PathGrant::repo_root(canonical("root"));
    let writable = PathGrant::agent_root(canonical("root/nested"), true, false);
    let decision = authorize(
        PathOperation::Write,
        &target,
        &[read_only.clone(), writable],
    )
    .unwrap();
    assert_eq!(decision.capabilities(), Capabilities::WRITE);
    assert_eq!(decision.persistence(), GrantPersistence::Persistent);
    assert_eq!(
        authorize(
            PathOperation::Write,
            &target,
            std::slice::from_ref(&read_only)
        ),
        Err(PathDenial::Unauthorized(PathOperation::Write))
    );
    assert!(authorize(PathOperation::Read, &target, &[read_only]).is_ok());
}

/// 会话与当次授权只覆盖精确文件和指定操作。
#[test]
fn session_and_one_shot_grants_do_not_expand() {
    let target = resolved("outside/document.md");
    let sibling = resolved("outside/other.md");
    let session = PathGrant::session_file_read(canonical("outside/document.md"));
    let decision = authorize_session(PathOperation::Read, &target, session.clone()).unwrap();
    assert_eq!(decision.persistence(), GrantPersistence::Session);
    for operation in [
        PathOperation::AutomaticRead,
        PathOperation::ResourceRead,
        PathOperation::TerminalCwd,
    ] {
        assert_eq!(
            authorize_session(operation, &target, session.clone()),
            Err(PathDenial::Unauthorized(operation))
        );
    }
    assert_eq!(
        authorize_session(PathOperation::Read, &sibling, session),
        Err(PathDenial::Unauthorized(PathOperation::Read))
    );

    let save_as = PathGrant::save_as_target(canonical("outside/document.md"));
    assert!(authorize_session(PathOperation::Write, &target, save_as.clone()).is_ok());
    assert_eq!(
        authorize_session(PathOperation::Read, &target, save_as),
        Err(PathDenial::Unauthorized(PathOperation::Read))
    );
    let output = PathGrant::output_target(canonical("outside/document.md"));
    let decision = authorize_session(PathOperation::Write, &target, output).unwrap();
    assert_eq!(decision.persistence(), GrantPersistence::OneShot);
}

/// 自动打开与 Markdown 资源读取只接受持久读授权。
#[test]
fn automatic_and_resource_reads_require_persistent_grants() {
    let target = resolved("root/file");
    for grant in [
        PathGrant::repo_root(canonical("root")),
        PathGrant::agent_root(canonical("root"), false, false),
    ] {
        assert!(
            authorize(
                PathOperation::AutomaticRead,
                &target,
                std::slice::from_ref(&grant)
            )
            .is_ok()
        );
        assert!(authorize(PathOperation::ResourceRead, &target, &[grant]).is_ok());
    }
}

/// 终端启动恢复同时需要终端与恢复能力。
#[test]
fn restored_terminal_cwd_requires_dedicated_capability() {
    let target = resolved("root");
    let project = PathGrant::project_root(canonical("root"));
    assert!(
        authorize(
            PathOperation::TerminalCwd,
            &target,
            std::slice::from_ref(&project)
        )
        .is_ok()
    );
    assert!(authorize(PathOperation::RestoreTerminalCwd, &target, &[project]).is_ok());

    let agent = PathGrant::agent_root(canonical("root"), false, true);
    assert!(
        authorize(
            PathOperation::TerminalCwd,
            &target,
            std::slice::from_ref(&agent)
        )
        .is_ok()
    );
    assert_eq!(
        authorize(PathOperation::RestoreTerminalCwd, &target, &[agent]),
        Err(PathDenial::Unauthorized(PathOperation::RestoreTerminalCwd))
    );
}

/// 内置集成授权不派生普通文件、目录或其他集成能力。
#[test]
fn integration_grants_cover_only_declared_files_and_operations() {
    let target = resolved("integration/hooks.json");
    let sibling = resolved("integration/other.json");
    let hooks = PathGrant::hooks_read_target(canonical("integration/hooks.json"));
    assert!(
        authorize(
            PathOperation::HooksRead,
            &target,
            std::slice::from_ref(&hooks)
        )
        .is_ok()
    );
    for operation in [PathOperation::Read, PathOperation::HooksWrite] {
        assert_eq!(
            authorize(operation, &target, std::slice::from_ref(&hooks)),
            Err(PathDenial::Unauthorized(operation))
        );
    }
    assert_eq!(
        authorize(PathOperation::HooksRead, &sibling, &[hooks]),
        Err(PathDenial::Unauthorized(PathOperation::HooksRead))
    );

    let history = PathGrant::history_quarantine_target(canonical("integration/hooks.json"));
    assert!(
        authorize(
            PathOperation::HistoryQuarantine,
            &target,
            std::slice::from_ref(&history)
        )
        .is_ok()
    );
    assert_eq!(
        authorize(PathOperation::HistoryRestore, &target, &[history]),
        Err(PathDenial::Unauthorized(PathOperation::HistoryRestore))
    );
    let restore = PathGrant::history_restore_target(canonical("integration/hooks.json"));
    assert!(authorize(PathOperation::HistoryRestore, &target, &[restore]).is_ok());
    let history_read = PathGrant::history_read_target(canonical("integration/hooks.json"));
    assert!(authorize(PathOperation::HistoryRead, &target, &[history_read]).is_ok());
    let hooks_write = PathGrant::hooks_write_target(canonical("integration/hooks.json"));
    assert!(authorize(PathOperation::HooksWrite, &target, &[hooks_write]).is_ok());
}

/// 同一词法路径重定向后按本次真实目标重新拒绝。
#[test]
fn redirected_target_does_not_inherit_old_path_authorization() {
    let grants = [PathGrant::project_root(canonical("root"))];
    let request = request(PathOperation::Write, absolute("root/link/file"));
    assert!(PathPolicy::authorize(&request, &resolved("root/inside/file"), &grants, 7).is_ok());
    assert_eq!(
        PathPolicy::authorize(&request, &resolved("outside/file"), &grants, 7),
        Err(PathDenial::Unauthorized(PathOperation::Write))
    );
}

/// 授权代次失配默认拒绝，不能用旧结果继续执行。
#[test]
fn stale_authorization_generation_is_rejected() {
    let grants = [PathGrant::project_root(canonical("root"))];
    let request = request(PathOperation::Write, absolute("root/file"));
    assert_eq!(
        PathPolicy::authorize(&request, &resolved("root/file"), &grants, 8),
        Err(PathDenial::GenerationMismatch {
            expected: 7,
            actual: 8,
        })
    );
    assert!(PathPolicy::authorize(&request, &resolved("root/file"), &grants, 7).is_ok());
}

/// 相对、盘符相对、设备与父目录跳转路径不进入隐式授权。
#[test]
fn unsafe_path_forms_are_rejected() {
    let grants = [PathGrant::project_root(canonical("root"))];
    for lexical in [PathBuf::from("relative/file"), PathBuf::from("C:relative")] {
        assert_eq!(
            PathPolicy::authorize(
                &request(PathOperation::Read, lexical),
                &resolved("root/file"),
                &grants,
                7,
            ),
            Err(PathDenial::InvalidPath(PathError::NotAbsolute))
        );
    }
    assert_eq!(CanonicalPath::new("relative"), Err(PathError::NotAbsolute));
    assert_eq!(
        CanonicalPath::new(absolute("root/../outside")),
        Err(PathError::NonCanonical)
    );
    if cfg!(windows) {
        assert_eq!(
            CanonicalPath::new(r"\\?\C:\root"),
            Err(PathError::DevicePath)
        );
    }
}

/// 根包含按路径组件判定，UNC 也必须有真实目标授权。
#[test]
fn containment_is_component_aware_and_unc_is_not_implicit() {
    let grants = [PathGrant::project_root(canonical("root"))];
    assert!(authorize(PathOperation::Read, &resolved("root/file"), &grants).is_ok());
    assert_eq!(
        authorize(PathOperation::Read, &resolved("root-other/file"), &grants),
        Err(PathDenial::Unauthorized(PathOperation::Read))
    );
    if cfg!(windows) {
        let unc_root = CanonicalPath::new("//server/share/root").unwrap();
        let unc_lexical = PathBuf::from("//server/share/root/file");
        let unc_target = ResolvedPath::new(CanonicalPath::new(unc_lexical.clone()).unwrap(), None);
        let unc_request = request(PathOperation::Read, unc_lexical);
        assert_eq!(
            PathPolicy::authorize(&unc_request, &unc_target, &[], 7),
            Err(PathDenial::Unauthorized(PathOperation::Read))
        );
        assert!(
            PathPolicy::authorize(
                &unc_request,
                &unc_target,
                &[PathGrant::repo_root(unc_root)],
                7,
            )
            .is_ok()
        );
    }
}

/// 决策携带本次解析结果、命中最小能力与持久性。
#[test]
fn authorization_carries_current_resolution_and_scoped_capabilities() {
    let target = resolved("outside/export.zip");
    let output = PathGrant::output_target(canonical("outside/export.zip"));
    let decision = authorize_session(PathOperation::Write, &target, output).unwrap();
    assert_eq!(decision.resolved_path(), &target);
    assert_eq!(
        decision.resolved_path().canonical_path(),
        &canonical("outside/export.zip")
    );
    assert_eq!(
        decision.resolved_path().file_identity(),
        target.file_identity()
    );
    assert_eq!(decision.capabilities(), Capabilities::WRITE);
    assert_eq!(decision.persistence(), GrantPersistence::OneShot);
}

/// 错误文本不插值路径内容，只说明封闭失败原因。
#[test]
fn path_errors_describe_closed_failure_reasons() {
    fn require_error<T: std::error::Error>() {}
    require_error::<PathError>();
    require_error::<PathDenial>();
    assert_eq!(PathError::NotAbsolute.to_string(), "路径必须为绝对路径");
    assert_eq!(
        PathError::DevicePath.to_string(),
        "设备路径不能作为授权输入"
    );
    assert_eq!(
        PathError::NonCanonical.to_string(),
        "规范路径不能包含父目录跳转"
    );
    assert_eq!(
        PathDenial::GenerationMismatch {
            expected: 7,
            actual: 8,
        }
        .to_string(),
        "授权代次已变化，必须重新解析与判定"
    );
    assert_eq!(
        PathDenial::Unauthorized(PathOperation::Write).to_string(),
        "路径未授予 Write 所需能力"
    );
    assert_eq!(
        PathDenial::InvalidPath(PathError::NotAbsolute).to_string(),
        "路径必须为绝对路径"
    );
}
