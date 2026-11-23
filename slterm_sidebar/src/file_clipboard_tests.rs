use std::str::FromStr;

use slterm_path::CanonicalPath;
use slterm_workspace::ProjectId;

use crate::{
    FileClipboard, FileClipboardItem, FileClipboardMode, FileTreeEntryKind, FileTreeRoot,
    PasteRejectionReason, PasteTarget,
};

fn path(value: &str) -> CanonicalPath {
    CanonicalPath::new(value).unwrap()
}

fn root(value: &str) -> FileTreeRoot {
    FileTreeRoot::Project(ProjectId::from_str(value).unwrap())
}

fn root_a() -> FileTreeRoot {
    root("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b")
}

fn root_b() -> FileTreeRoot {
    root("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c")
}

fn item(root: FileTreeRoot, value: &str, kind: FileTreeEntryKind) -> FileClipboardItem {
    FileClipboardItem {
        root,
        path: path(value),
        kind,
    }
}

#[test]
fn explorer_rejects_cross_root_cut_and_descendant_paste() {
    let clipboard = FileClipboard::new(
        FileClipboardMode::Cut,
        vec![item(root_a(), "D:/a/file.txt", FileTreeEntryKind::File)],
    );
    let plan = clipboard.plan_paste(PasteTarget {
        root: root_b(),
        directory: path("D:/b/out"),
    });
    assert!(plan.items.is_empty());
    assert_eq!(
        plan.rejections[0].reason,
        PasteRejectionReason::CrossRootCut
    );

    let clipboard = FileClipboard::new(
        FileClipboardMode::Cut,
        vec![
            item(root_a(), "D:/a/dir", FileTreeEntryKind::Directory),
            item(root_a(), "D:/a/good.txt", FileTreeEntryKind::File),
        ],
    );
    let plan = clipboard.plan_paste(PasteTarget {
        root: root_a(),
        directory: path("D:/a/dir/nested"),
    });
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].target, path("D:/a/dir/nested/good.txt"));
    assert_eq!(
        plan.rejections[0].reason,
        PasteRejectionReason::DescendantPaste
    );
}

#[test]
fn file_clipboard_copy_allows_cross_roots_and_keeps_only_topmost_items() {
    let clipboard = FileClipboard::new(
        FileClipboardMode::Copy,
        vec![
            item(root_a(), "D:/a/docs/guide.md", FileTreeEntryKind::File),
            item(root_a(), "D:/a/docs", FileTreeEntryKind::Directory),
            item(root_a(), "D:/a/docs", FileTreeEntryKind::Directory),
        ],
    );
    let plan = clipboard.plan_paste(PasteTarget {
        root: root_b(),
        directory: path("D:/b/incoming"),
    });

    assert!(plan.rejections.is_empty());
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].source.path, path("D:/a/docs"));
    assert_eq!(plan.items[0].target, path("D:/b/incoming/docs"));
}

#[test]
fn file_clipboard_rejects_same_directory_and_duplicate_destinations() {
    let clipboard = FileClipboard::new(
        FileClipboardMode::Copy,
        vec![item(root_a(), "D:/a/direct.txt", FileTreeEntryKind::File)],
    );
    let plan = clipboard.plan_paste(PasteTarget {
        root: root_a(),
        directory: path("D:/a"),
    });
    assert_eq!(
        plan.rejections[0].reason,
        PasteRejectionReason::SameDirectory
    );

    let clipboard = FileClipboard::new(
        FileClipboardMode::Copy,
        vec![
            item(root_a(), "D:/a/one/same.txt", FileTreeEntryKind::File),
            item(root_a(), "D:/a/two/SAME.txt", FileTreeEntryKind::File),
        ],
    );
    let plan = clipboard.plan_paste(PasteTarget {
        root: root_b(),
        directory: path("D:/b/out"),
    });
    assert_eq!(plan.items.len(), 1);
    assert_eq!(
        plan.rejections[0].reason,
        PasteRejectionReason::DuplicateDestination
    );

    let clipboard = FileClipboard::new(
        FileClipboardMode::Copy,
        vec![item(root_a(), "D:/", FileTreeEntryKind::File)],
    );
    let plan = clipboard.plan_paste(PasteTarget {
        root: root_b(),
        directory: path("D:/b/out"),
    });
    assert_eq!(
        plan.rejections[0].reason,
        PasteRejectionReason::InvalidSource
    );
}
