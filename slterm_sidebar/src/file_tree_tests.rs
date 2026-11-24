use std::str::FromStr;

use slterm_path::CanonicalPath;
use slterm_workspace::ProjectId;

use crate::{
    DirectoryReadRequest, DirectoryStatus, FileTreeEntry, FileTreeEntryKind, FileTreeError,
    FileTreeModel, FileTreeRoot,
};

fn path(value: &str) -> CanonicalPath {
    CanonicalPath::new(value).unwrap()
}

fn model() -> FileTreeModel {
    FileTreeModel::new(
        FileTreeRoot::Project(ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b").unwrap()),
        path("D:/work/project"),
    )
}

fn entry(value: &str, kind: FileTreeEntryKind) -> FileTreeEntry {
    FileTreeEntry::new(path(value), kind)
}

fn file(value: &str) -> FileTreeEntry {
    entry(value, FileTreeEntryKind::File)
}

fn directory(value: &str) -> FileTreeEntry {
    entry(value, FileTreeEntryKind::Directory)
}

fn apply(
    tree: &mut FileTreeModel,
    request: &DirectoryReadRequest,
    entries: Vec<FileTreeEntry>,
    complete: bool,
) -> Option<DirectoryReadRequest> {
    tree.apply_batch(request, entries, complete).unwrap()
}

#[test]
fn file_tree_batches_have_stable_directory_first_order() {
    let mut tree = model();
    let root = path("D:/work/project");
    let first = tree.request_directory(&root).unwrap();
    let next = apply(
        &mut tree,
        &first,
        vec![
            file("D:/work/project/z.txt"),
            directory("D:/work/project/B"),
            file("D:/work/project/a.txt"),
        ],
        false,
    )
    .unwrap();
    assert_eq!(next.offset, 3);
    assert_eq!(
        tree.visible_rows()
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        ["B", "a.txt", "z.txt"]
    );

    apply(&mut tree, &next, Vec::new(), true);
    assert_eq!(tree.directory_status(&root), DirectoryStatus::Loaded);
}

#[test]
fn file_tree_filters_hidden_git_and_enforces_direct_child_boundaries() {
    let mut tree = model();
    let root = path("D:/work/project");
    let request = tree.request_directory(&root).unwrap();
    apply(
        &mut tree,
        &request,
        vec![
            directory("D:/work/project/.git"),
            file("D:/work/project/.env"),
        ],
        true,
    );
    assert_eq!(
        tree.visible_rows()
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        [".env"]
    );

    let request = tree.invalidate(&root).unwrap().unwrap();
    let error = tree
        .apply_batch(
            &request,
            vec![file("D:/work/project/nested/escape.txt")],
            true,
        )
        .unwrap_err();
    assert_eq!(error, FileTreeError::NotDirectChild);
    assert!(tree.visible_rows().is_empty());

    let outside = tree.request_directory(&path("D:/work/other"));
    assert_eq!(outside, None);
}

#[test]
fn file_tree_expand_select_and_filter_share_visible_projection() {
    let mut tree = model();
    let root = path("D:/work/project");
    let docs = path("D:/work/project/docs");
    let guide = path("D:/work/project/docs/guide.md");
    let request = tree.request_directory(&root).unwrap();
    apply(
        &mut tree,
        &request,
        vec![
            directory("D:/work/project/docs"),
            file("D:/work/project/README.md"),
        ],
        true,
    );

    let request = tree.expand(&docs).unwrap().unwrap();
    apply(
        &mut tree,
        &request,
        vec![file("D:/work/project/docs/guide.md")],
        true,
    );
    assert_eq!(
        tree.visible_rows()
            .iter()
            .map(|row| (row.name.as_str(), row.depth))
            .collect::<Vec<_>>(),
        [("docs", 0), ("guide.md", 1), ("README.md", 0)]
    );
    tree.select(&guide).unwrap();
    assert!(
        tree.visible_rows()
            .iter()
            .any(|row| row.path == guide && row.selected)
    );
    tree.set_filter("README");
    assert!(tree.visible_rows().iter().all(|row| !row.selected));
    tree.set_filter("");
    assert!(tree.visible_rows().iter().all(|row| !row.selected));

    tree.set_filter("guide");
    assert_eq!(
        tree.visible_rows()
            .iter()
            .map(|row| (row.name.as_str(), row.depth))
            .collect::<Vec<_>>(),
        [("docs", 0), ("guide.md", 1)]
    );
    tree.set_filter("docs/guide");
    assert!(tree.visible_rows().is_empty());
    tree.set_filter("");
    tree.collapse(&docs).unwrap();
    assert_eq!(
        tree.visible_rows()
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        ["docs", "README.md"]
    );
}

#[test]
fn file_tree_partial_failure_keeps_read_rows_and_reports_root_failure() {
    let mut tree = model();
    let root = path("D:/work/project");
    let first = tree.request_directory(&root).unwrap();
    let next = apply(
        &mut tree,
        &first,
        vec![file("D:/work/project/kept.txt")],
        false,
    )
    .unwrap();
    tree.fail_directory(&next).unwrap();
    assert_eq!(
        tree.directory_status(&root),
        DirectoryStatus::Failed { partial: true }
    );
    assert_eq!(tree.visible_rows()[0].name, "kept.txt");

    let mut empty = model();
    let root = path("D:/work/project");
    let request = empty.request_directory(&root).unwrap();
    empty.fail_directory(&request).unwrap();
    assert_eq!(
        empty.directory_status(&root),
        DirectoryStatus::Failed { partial: false }
    );
}

#[test]
fn file_tree_refresh_preserves_old_rows_on_failure_and_prunes_after_success() {
    let mut tree = model();
    let root = path("D:/work/project");
    let old = path("D:/work/project/old.txt");
    let docs = path("D:/work/project/docs");
    let guide = path("D:/work/project/docs/guide.md");
    let request = tree.request_directory(&root).unwrap();
    apply(
        &mut tree,
        &request,
        vec![
            directory("D:/work/project/docs"),
            file("D:/work/project/old.txt"),
            file("D:/work/project/keep.txt"),
        ],
        true,
    );
    let request = tree.expand(&docs).unwrap().unwrap();
    apply(
        &mut tree,
        &request,
        vec![file("D:/work/project/docs/guide.md")],
        true,
    );
    tree.select(&old).unwrap();

    let request = tree.refresh(&root).unwrap().unwrap();
    assert!(tree.visible_rows().iter().any(|row| row.path == guide));
    assert!(
        tree.visible_rows()
            .iter()
            .any(|row| row.path == old && row.selected)
    );
    tree.fail_directory(&request).unwrap();
    assert!(tree.visible_rows().iter().any(|row| row.path == guide));
    assert!(
        tree.visible_rows()
            .iter()
            .any(|row| row.path == old && row.selected)
    );

    let request = tree.refresh(&root).unwrap().unwrap();
    apply(
        &mut tree,
        &request,
        vec![
            directory("D:/work/project/docs"),
            file("D:/work/project/keep.txt"),
            file("D:/work/project/new.txt"),
        ],
        true,
    );
    assert_eq!(
        tree.visible_rows()
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        ["docs", "guide.md", "keep.txt", "new.txt"]
    );
    assert!(tree.visible_rows().iter().all(|row| !row.selected));
}

#[test]
fn file_tree_rejects_stale_root_and_directory_read_generations() {
    let mut tree = model();
    let root = path("D:/work/project");
    let old = tree.request_directory(&root).unwrap();
    let current = tree.invalidate(&root).unwrap().unwrap();
    assert_ne!(old.read_generation, current.read_generation);
    assert_eq!(
        tree.apply_batch(&old, vec![file("D:/work/project/stale.txt")], true),
        Err(FileTreeError::StaleRead)
    );
    apply(
        &mut tree,
        &current,
        vec![file("D:/work/project/current.txt")],
        true,
    );

    let old_root = current;
    tree.replace_root(
        FileTreeRoot::Project(ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b").unwrap()),
        path("D:/work/new-root"),
    );
    assert_eq!(
        tree.apply_batch(&old_root, Vec::new(), true),
        Err(FileTreeError::StaleRoot)
    );
    assert!(tree.visible_rows().is_empty());
}

/// 恢复快照只导出可见投影中的展开目录与当前选择，不伪造未加载节点。
#[test]
fn file_tree_exposes_expanded_and_selected_paths_for_restore() {
    let mut tree = model();
    let root = path("D:/work/project");
    let docs = path("D:/work/project/docs");
    let sub = path("D:/work/project/docs/sub");
    let guide = path("D:/work/project/docs/guide.md");
    let request = tree.request_directory(&root).unwrap();
    apply(
        &mut tree,
        &request,
        vec![
            directory("D:/work/project/docs"),
            file("D:/work/project/a.txt"),
        ],
        true,
    );
    assert_eq!(tree.expanded_paths(), Vec::<CanonicalPath>::new());
    assert_eq!(tree.kind_of(&docs), Some(FileTreeEntryKind::Directory));
    assert_eq!(tree.kind_of(&guide), None, "未加载节点不得伪造类型");
    assert_eq!(tree.selected(), None);
    assert!(!tree.is_expanded(&docs));

    let request = tree.expand(&docs).unwrap().unwrap();
    apply(
        &mut tree,
        &request,
        vec![
            directory("D:/work/project/docs/sub"),
            file("D:/work/project/docs/guide.md"),
        ],
        true,
    );
    let request = tree.expand(&sub).unwrap().unwrap();
    apply(&mut tree, &request, Vec::new(), true);
    assert_eq!(tree.expanded_paths(), vec![docs.clone(), sub.clone()]);
    assert!(tree.is_expanded(&docs));
    assert!(tree.is_expanded(&sub));

    tree.select(&guide).unwrap();
    assert_eq!(tree.selected(), Some(&guide));

    tree.collapse(&docs).unwrap();
    assert!(!tree.is_expanded(&docs));
    assert_eq!(
        tree.expanded_paths(),
        Vec::<CanonicalPath>::new(),
        "折叠祖先不得把不可见后代导出为恢复快照"
    );
}
