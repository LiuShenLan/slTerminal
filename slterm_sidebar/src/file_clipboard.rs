//! 应用会话级文件剪贴板与纯粘贴目标转换；不读取文件系统或系统剪贴板。

use std::collections::BTreeSet;

use slterm_path::CanonicalPath;

use crate::{FileTreeEntryKind, FileTreeRoot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileClipboardItem {
    pub root: FileTreeRoot,
    pub path: CanonicalPath,
    pub kind: FileTreeEntryKind,
}

#[derive(Clone, Debug)]
pub struct FileClipboard {
    mode: FileClipboardMode,
    items: Vec<FileClipboardItem>,
}

impl FileClipboard {
    pub fn new(mode: FileClipboardMode, items: Vec<FileClipboardItem>) -> Self {
        Self {
            mode,
            items: topmost_items(items),
        }
    }

    pub fn plan_paste(&self, target: PasteTarget) -> PastePlan {
        let mut plan = PastePlan {
            mode: self.mode,
            target,
            items: Vec::new(),
            rejections: Vec::new(),
        };
        let mut destination_names = BTreeSet::new();
        for source in &self.items {
            let reason = if self.mode == FileClipboardMode::Cut && source.root != plan.target.root {
                Some(PasteRejectionReason::CrossRootCut)
            } else if source.kind == FileTreeEntryKind::Directory
                && plan
                    .target
                    .directory
                    .as_path()
                    .starts_with(source.path.as_path())
            {
                Some(PasteRejectionReason::DescendantPaste)
            } else if source.path.as_path().parent() == Some(plan.target.directory.as_path()) {
                Some(PasteRejectionReason::SameDirectory)
            } else {
                None
            };
            if let Some(reason) = reason {
                plan.rejections.push(PasteRejection {
                    source: source.clone(),
                    reason,
                });
                continue;
            }
            let Some(name) = source.path.as_path().file_name() else {
                plan.rejections.push(PasteRejection {
                    source: source.clone(),
                    reason: PasteRejectionReason::InvalidSource,
                });
                continue;
            };
            let destination_key = name.to_string_lossy().to_lowercase();
            if !destination_names.insert(destination_key) {
                plan.rejections.push(PasteRejection {
                    source: source.clone(),
                    reason: PasteRejectionReason::DuplicateDestination,
                });
                continue;
            }
            let target_path = CanonicalPath::new(plan.target.directory.as_path().join(name))
                .expect("既有规范目录与单段子项必须组成规范路径");
            plan.items.push(PasteItem {
                source: source.clone(),
                target: target_path,
            });
        }
        plan
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasteTarget {
    pub root: FileTreeRoot,
    pub directory: CanonicalPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasteItem {
    pub source: FileClipboardItem,
    pub target: CanonicalPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasteRejection {
    pub source: FileClipboardItem,
    pub reason: PasteRejectionReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteRejectionReason {
    CrossRootCut,
    DescendantPaste,
    SameDirectory,
    DuplicateDestination,
    InvalidSource,
}

#[derive(Clone, Debug)]
pub struct PastePlan {
    pub mode: FileClipboardMode,
    pub target: PasteTarget,
    pub items: Vec<PasteItem>,
    pub rejections: Vec<PasteRejection>,
}

fn topmost_items(items: Vec<FileClipboardItem>) -> Vec<FileClipboardItem> {
    let mut topmost = Vec::new();
    for item in items {
        let covered = topmost.iter().any(|existing: &FileClipboardItem| {
            existing.root == item.root
                && (existing.path == item.path
                    || existing.kind == FileTreeEntryKind::Directory
                        && item.path.as_path().starts_with(existing.path.as_path()))
        });
        if covered {
            continue;
        }
        if item.kind == FileTreeEntryKind::Directory {
            topmost.retain(|existing| {
                existing.root != item.root
                    || !existing.path.as_path().starts_with(item.path.as_path())
            });
        }
        topmost.push(item);
    }
    topmost
}
