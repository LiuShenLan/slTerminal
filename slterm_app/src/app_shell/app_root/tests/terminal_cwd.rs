//! 真实项目路径下的终端 cwd 授权集成。

use super::*;

#[gpui::test]
fn integration_new_terminal_cwd(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let nested = project_root.join("nested");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let (root, mut window, state) = open_root(cx);
    let project = open_generated_project(&root, &mut window, project_root.to_str().unwrap());
    let project_root = root.read_with(cx, |root, _| {
        root.snapshot
            .registry()
            .project(project)
            .unwrap()
            .root()
            .clone()
    });

    let mut spec = terminal_spec(PROJECT_A, GROUP_A, TERMINAL_A);
    spec.project_id = project;
    let created = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal_with_identity(spec, window, cx)
        })
    });
    assert!(created.is_ok());
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.snapshot
                .terminal(terminal_id(TERMINAL_A))
                .unwrap()
                .cwd(),
            &project_root
        );
    });

    let mut nested_spec = terminal_spec(PROJECT_A, GROUP_A, TERMINAL_B);
    nested_spec.project_id = project;
    nested_spec.cwd = Some(path(nested.to_str().unwrap()));
    let created = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal_with_identity(nested_spec, window, cx)
        })
    });
    assert!(created.is_ok());
    root.read_with(cx, |root, _| {
        assert_eq!(
            root.snapshot
                .terminal(terminal_id(TERMINAL_B))
                .unwrap()
                .cwd()
                .as_path(),
            nested.as_path()
        );
    });

    let before = root.read_with(cx, |root, _| root.snapshot().clone());
    let attempts = state.borrow().attempts;
    let mut rejected_spec = terminal_spec(PROJECT_A, GROUP_A, TERMINAL_C);
    rejected_spec.project_id = project;
    rejected_spec.cwd = Some(path(outside.to_str().unwrap()));
    let rejected = window.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.new_terminal_with_identity(rejected_spec, window, cx)
        })
    });
    assert!(rejected.is_err());
    assert_eq!(
        root.read_with(cx, |root, _| root.snapshot().clone()),
        before
    );
    assert_eq!(
        state.borrow().attempts,
        attempts,
        "越权 cwd 不得启动 worker"
    );
}
