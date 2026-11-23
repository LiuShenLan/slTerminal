//! 纯文件树模型；目录读取由调用方执行，模型只校验回执并维护投影。

use std::collections::{BTreeMap, BTreeSet};

use slterm_path::CanonicalPath;
use slterm_settings::AgentRootId;
use slterm_workspace::ProjectId;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileTreeRoot {
    Project(ProjectId),
    Agent(AgentRootId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileTreeEntryKind {
    Directory,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTreeEntry {
    pub path: CanonicalPath,
    pub kind: FileTreeEntryKind,
}

impl FileTreeEntry {
    pub fn new(path: CanonicalPath, kind: FileTreeEntryKind) -> Self {
        Self { path, kind }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryReadRequest {
    pub root_generation: u64,
    pub read_generation: u64,
    pub directory: CanonicalPath,
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DirectoryStatus {
    #[default]
    NotRequested,
    Loading,
    Loaded,
    Failed {
        partial: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTreeRow {
    pub path: CanonicalPath,
    pub name: String,
    pub depth: usize,
    pub kind: FileTreeEntryKind,
    pub selected: bool,
    pub directory_status: Option<DirectoryStatus>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileTreeError {
    NotFound,
    NotDirectory,
    StaleRoot,
    StaleRead,
    OutsideRoot,
    NotDirectChild,
    DuplicateEntry,
}

#[derive(Debug)]
struct FileTreeNode {
    name: String,
    kind: FileTreeEntryKind,
    expanded: bool,
    children: Vec<CanonicalPath>,
    received: usize,
    read_generation: u64,
    status: DirectoryStatus,
}

impl FileTreeNode {
    fn root() -> Self {
        Self {
            name: String::new(),
            kind: FileTreeEntryKind::Directory,
            expanded: true,
            children: Vec::new(),
            received: 0,
            read_generation: 0,
            status: DirectoryStatus::NotRequested,
        }
    }

    fn from_entry(entry: &FileTreeEntry) -> Result<Self, FileTreeError> {
        let name = entry
            .path
            .as_path()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .ok_or(FileTreeError::NotDirectChild)?;
        Ok(Self {
            name,
            kind: entry.kind,
            expanded: false,
            children: Vec::new(),
            received: 0,
            read_generation: 0,
            status: DirectoryStatus::NotRequested,
        })
    }
}

#[derive(Debug)]
pub struct FileTreeModel {
    root: FileTreeRoot,
    root_path: CanonicalPath,
    root_generation: u64,
    nodes: BTreeMap<CanonicalPath, FileTreeNode>,
    selected: Option<CanonicalPath>,
    filter: String,
}

impl FileTreeModel {
    pub fn new(root: FileTreeRoot, root_path: CanonicalPath) -> Self {
        let nodes = BTreeMap::from([(root_path.clone(), FileTreeNode::root())]);
        Self {
            root,
            root_path,
            root_generation: 0,
            nodes,
            selected: None,
            filter: String::new(),
        }
    }

    pub fn root(&self) -> &FileTreeRoot {
        &self.root
    }

    pub fn root_path(&self) -> &CanonicalPath {
        &self.root_path
    }

    pub fn replace_root(&mut self, root: FileTreeRoot, root_path: CanonicalPath) {
        self.root_generation = self
            .root_generation
            .checked_add(1)
            .expect("文件树根代次不得耗尽");
        self.root = root;
        self.root_path = root_path.clone();
        self.nodes = BTreeMap::from([(root_path, FileTreeNode::root())]);
        self.selected = None;
        self.filter.clear();
    }

    fn request_for(&self, directory: &CanonicalPath, node: &FileTreeNode) -> DirectoryReadRequest {
        DirectoryReadRequest {
            root_generation: self.root_generation,
            read_generation: node.read_generation,
            directory: directory.clone(),
            offset: node.received,
        }
    }

    pub fn request_directory(&mut self, directory: &CanonicalPath) -> Option<DirectoryReadRequest> {
        let node = self.nodes.get(directory)?;
        if node.kind != FileTreeEntryKind::Directory
            || !node.expanded
            || matches!(
                node.status,
                DirectoryStatus::Loading | DirectoryStatus::Loaded
            )
        {
            return None;
        }
        let request = self.request_for(directory, node);
        self.nodes
            .get_mut(directory)
            .expect("请求节点必须存在")
            .status = DirectoryStatus::Loading;
        Some(request)
    }

    pub fn expand(
        &mut self,
        directory: &CanonicalPath,
    ) -> Result<Option<DirectoryReadRequest>, FileTreeError> {
        let node = self
            .nodes
            .get_mut(directory)
            .ok_or(FileTreeError::NotFound)?;
        if node.kind != FileTreeEntryKind::Directory {
            return Err(FileTreeError::NotDirectory);
        }
        node.expanded = true;
        Ok(self.request_directory(directory))
    }

    pub fn collapse(&mut self, directory: &CanonicalPath) -> Result<(), FileTreeError> {
        let node = self
            .nodes
            .get_mut(directory)
            .ok_or(FileTreeError::NotFound)?;
        if node.kind != FileTreeEntryKind::Directory {
            return Err(FileTreeError::NotDirectory);
        }
        node.expanded = false;
        Ok(())
    }

    pub fn invalidate(
        &mut self,
        directory: &CanonicalPath,
    ) -> Result<Option<DirectoryReadRequest>, FileTreeError> {
        let node = self
            .nodes
            .get_mut(directory)
            .ok_or(FileTreeError::NotFound)?;
        if node.kind != FileTreeEntryKind::Directory {
            return Err(FileTreeError::NotDirectory);
        }
        let removed = std::mem::take(&mut node.children);
        for child in removed {
            self.remove_subtree(&child);
        }
        let node = self.nodes.get_mut(directory).expect("失效目录必须保留");
        node.received = 0;
        node.status = DirectoryStatus::NotRequested;
        node.read_generation = node
            .read_generation
            .checked_add(1)
            .expect("目录读取代次不得耗尽");
        if self
            .selected
            .as_ref()
            .is_some_and(|path| !self.nodes.contains_key(path))
        {
            self.selected = None;
        }
        Ok(self.request_directory(directory))
    }

    fn remove_subtree(&mut self, path: &CanonicalPath) {
        if let Some(node) = self.nodes.remove(path) {
            for child in node.children {
                self.remove_subtree(&child);
            }
        }
    }

    pub fn fail_directory(&mut self, request: &DirectoryReadRequest) -> Result<(), FileTreeError> {
        self.validate_request(request)?;
        let node = self
            .nodes
            .get_mut(&request.directory)
            .expect("请求目录必须存在");
        node.status = DirectoryStatus::Failed {
            partial: node.received > 0,
        };
        Ok(())
    }

    fn validate_request(&self, request: &DirectoryReadRequest) -> Result<(), FileTreeError> {
        if request.root_generation != self.root_generation {
            return Err(FileTreeError::StaleRoot);
        }
        let node = self
            .nodes
            .get(&request.directory)
            .ok_or(FileTreeError::NotFound)?;
        if node.kind != FileTreeEntryKind::Directory {
            return Err(FileTreeError::NotDirectory);
        }
        if node.read_generation != request.read_generation
            || node.received != request.offset
            || node.status != DirectoryStatus::Loading
        {
            return Err(FileTreeError::StaleRead);
        }
        Ok(())
    }

    pub fn apply_batch(
        &mut self,
        request: &DirectoryReadRequest,
        entries: Vec<FileTreeEntry>,
        complete: bool,
    ) -> Result<Option<DirectoryReadRequest>, FileTreeError> {
        self.validate_request(request)?;
        let mut batch_paths = BTreeSet::new();
        for entry in &entries {
            if !entry.path.as_path().starts_with(self.root_path.as_path()) {
                return Err(FileTreeError::OutsideRoot);
            }
            if entry.path.as_path().parent() != Some(request.directory.as_path()) {
                return Err(FileTreeError::NotDirectChild);
            }
            if self.nodes.contains_key(&entry.path) || !batch_paths.insert(entry.path.clone()) {
                return Err(FileTreeError::DuplicateEntry);
            }
            FileTreeNode::from_entry(entry)?;
        }

        let mut visible_entries = Vec::new();
        for entry in &entries {
            let name = entry.path.as_path().file_name().expect("名称已校验");
            if name.to_string_lossy().eq_ignore_ascii_case(".git") {
                continue;
            }
            visible_entries.push(FileTreeNode::from_entry(entry)?);
        }
        let node = self
            .nodes
            .get_mut(&request.directory)
            .expect("请求目录必须存在");
        node.received += entries.len();
        for (entry, child) in entries
            .iter()
            .filter(|entry| {
                !entry
                    .path
                    .as_path()
                    .file_name()
                    .expect("名称已校验")
                    .to_string_lossy()
                    .eq_ignore_ascii_case(".git")
            })
            .zip(visible_entries)
        {
            self.nodes.insert(entry.path.clone(), child);
            self.nodes
                .get_mut(&request.directory)
                .expect("请求目录必须存在")
                .children
                .push(entry.path.clone());
        }
        self.sort_children(&request.directory);
        let node = self
            .nodes
            .get_mut(&request.directory)
            .expect("请求目录必须存在");
        if complete {
            node.status = DirectoryStatus::Loaded;
            Ok(None)
        } else {
            Ok(Some(DirectoryReadRequest {
                root_generation: self.root_generation,
                read_generation: node.read_generation,
                directory: request.directory.clone(),
                offset: node.received,
            }))
        }
    }

    fn sort_children(&mut self, directory: &CanonicalPath) {
        let mut children = self.nodes[directory].children.clone();
        children.sort_by(|left, right| {
            let left_node = &self.nodes[left];
            let right_node = &self.nodes[right];
            left_node
                .kind
                .directory_rank()
                .cmp(&right_node.kind.directory_rank())
                .then_with(|| {
                    left_node
                        .name
                        .to_lowercase()
                        .cmp(&right_node.name.to_lowercase())
                })
                .then_with(|| left_node.name.cmp(&right_node.name))
                .then_with(|| left.cmp(right))
        });
        self.nodes
            .get_mut(directory)
            .expect("排序目录必须存在")
            .children = children;
    }

    pub fn directory_status(&self, directory: &CanonicalPath) -> DirectoryStatus {
        self.nodes
            .get(directory)
            .map(|node| node.status)
            .unwrap_or(DirectoryStatus::NotRequested)
    }

    pub fn select(&mut self, path: &CanonicalPath) -> Result<(), FileTreeError> {
        if !self.visible_rows().iter().any(|row| &row.path == path) {
            return Err(FileTreeError::NotFound);
        }
        self.selected = Some(path.clone());
        Ok(())
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.trim().to_lowercase();
    }

    pub fn visible_rows(&self) -> Vec<FileTreeRow> {
        let mut rows = Vec::new();
        self.collect_visible(&self.root_path.clone(), 0, &mut rows);
        rows
    }

    fn collect_visible(
        &self,
        directory: &CanonicalPath,
        depth: usize,
        rows: &mut Vec<FileTreeRow>,
    ) {
        let Some(node) = self.nodes.get(directory) else {
            return;
        };
        for path in &node.children {
            let Some(child) = self.nodes.get(path) else {
                continue;
            };
            let matches = self.matches_filter(path, child);
            let mut descendants = Vec::new();
            if child.kind == FileTreeEntryKind::Directory
                && (self.filter.is_empty() && child.expanded || !self.filter.is_empty())
            {
                self.collect_visible(path, depth + 1, &mut descendants);
            }
            if self.filter.is_empty() || matches || !descendants.is_empty() {
                rows.push(FileTreeRow {
                    path: path.clone(),
                    name: child.name.clone(),
                    depth,
                    kind: child.kind,
                    selected: self.selected.as_ref() == Some(path),
                    directory_status: (child.kind == FileTreeEntryKind::Directory)
                        .then_some(child.status),
                });
                rows.extend(descendants);
            }
        }
    }

    fn matches_filter(&self, path: &CanonicalPath, node: &FileTreeNode) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let relative = path
            .as_path()
            .strip_prefix(self.root_path.as_path())
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase();
        node.name.to_lowercase().contains(&self.filter) || relative.contains(&self.filter)
    }
}

impl FileTreeEntryKind {
    fn directory_rank(self) -> u8 {
        match self {
            Self::Directory => 0,
            Self::File => 1,
        }
    }
}
