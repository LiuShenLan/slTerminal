//! Agent 根快照与 Document 打开授权：根集合派生、后台加载与文件树同步。
//!
//! 根快照只读派生自 `agent_global_dirs` + 自定义根注册表 + `agent_files`
//! 展示配置；授权只按已解析真实目录计算，前台不触碰 home 与设置文件。

use gpui::{Context, Window};
use slterm_path::{CanonicalPath, PathGrant};
use slterm_settings::{AgentRootCapabilities, AgentRootId, AgentRootVisibility};
use slterm_workspace::SideViewId;

use super::AppRoot;
use super::task_service::{CancellationToken, TaskId};

impl AppRoot {
    /// Agent 树文件打开：活跃项目聚焦组 + 统一 Document 打开链。
    pub(super) fn handle_agent_files_open(
        &mut self,
        path: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(project_id) = self.snapshot.registry().active() else {
            return;
        };
        let target_group = match self.focused_group(project_id, cx) {
            Ok(group) => group,
            Err(_) => return,
        };
        let mut request = match self.prepare_document_open(project_id, target_group, path, false) {
            Ok(request) => request,
            Err(_) => return,
        };
        request.read_grants = self.agent_root_grants_for_path(&request.path);
        self.queue_document_open(request, None, window, cx);
    }

    /// 覆盖该路径的 Agent 根只读／可写授权；根快照未就绪时为空。
    pub(super) fn agent_root_grants_for_path(&self, path: &CanonicalPath) -> Vec<PathGrant> {
        let Some(registry) = self.agent_root_registry.as_ref() else {
            return Vec::new();
        };
        let mut grants = Vec::new();
        for (_, _, root_path) in registry.builtin_roots() {
            if path.as_path().starts_with(root_path.as_path()) {
                // 内置根固定只读、禁止终端。
                grants.push(PathGrant::agent_root(root_path.clone(), false, false));
            }
        }
        for (root, root_path) in registry.custom_roots_with_real() {
            if path.as_path().starts_with(root_path.as_path()) {
                grants.push(PathGrant::agent_root(
                    root_path.clone(),
                    root.capabilities().allows_write(),
                    root.capabilities().allows_terminal(),
                ));
            }
        }
        grants
    }

    /// Document 打开／保存授权：项目根并集再叠加 Agent 根能力。
    pub(super) fn document_grants(
        &self,
        document_id: slterm_document::DocumentId,
    ) -> Vec<PathGrant> {
        let mut grants =
            crate::app_shell::documents::document_project_grants(&self.snapshot, document_id);
        let path = self
            .snapshot
            .documents()
            .find(|(id, _)| *id == document_id)
            .map(|(_, location)| location.canonical_path().clone());
        if let Some(path) = path {
            grants.extend(self.agent_root_grants_for_path(&path));
        }
        grants
    }

    /// 视图打开/根同步：安装运行态并交给视图自行读取。
    pub(super) fn sync_agent_files_roots(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let roots = self.agent_files_roots();
        let Some(view) = self.side_views.instance(SideViewId::AgentFiles).cloned() else {
            return;
        };
        view.update(cx, |view, cx| {
            view.ensure_agent_files_filter_input(window, cx);
            view.agent_files_sync(roots, cx);
        });
    }

    /// 后台根快照回投：只同步根与读取，输入框由下一次带窗口的同步安装。
    pub(super) fn sync_agent_files_roots_after_load(&mut self, cx: &mut Context<'_, Self>) {
        let roots = self.agent_files_roots();
        let Some(view) = self.side_views.instance(SideViewId::AgentFiles).cloned() else {
            return;
        };
        view.update(cx, |view, cx| view.agent_files_sync(roots, cx));
        self.reconcile_fs_watch();
    }

    pub(super) fn agent_files_roots(
        &self,
    ) -> Vec<(
        AgentRootId,
        String,
        CanonicalPath,
        AgentRootVisibility,
        AgentRootCapabilities,
    )> {
        if !self.agent_roots_loaded {
            return Vec::new();
        }
        let Some(registry) = self.agent_root_registry.as_ref() else {
            return Vec::new();
        };
        let mut roots = self
            .agent_global_dirs
            .iter()
            .flatten()
            .filter_map(|dir| {
                let lexical = CanonicalPath::new(dir.path.clone()).ok()?;
                let path = registry
                    .builtin_real(dir.root_id)
                    .cloned()
                    .unwrap_or(lexical);
                Some((
                    dir.root_id,
                    dir.display_name.clone(),
                    path,
                    self.agent_files_config
                        .visibility(Some(dir.cli_id.as_str()), dir.root_id),
                    // 内置根固定只读、禁止终端。
                    AgentRootCapabilities::READ_ONLY,
                ))
            })
            .collect::<Vec<_>>();
        roots.extend(registry.custom_roots_with_real().map(|(root, real)| {
            (
                root.id(),
                root.display_name().to_owned(),
                real.clone(),
                self.agent_files_config.visibility(None, root.id()),
                root.capabilities(),
            )
        }));
        roots
    }

    /// 首帧后单任务加载 Agent 根快照；前台不触碰 home 与设置文件。
    pub(super) fn ensure_agent_roots_loading(&mut self, cx: &mut Context<'_, Self>) {
        if self.agent_roots_loaded || self.tasks.is_registered(TaskId::AgentRootsLoad) {
            return;
        }
        let executor = cx.background_executor().clone();
        self.tasks
            .register_with(
                TaskId::AgentRootsLoad,
                CancellationToken::new(),
                move || {
                    let loading = executor.spawn(async move {
                        let dirs = crate::agent_dirs::agent_global_dirs();
                        let (roots, read_error) =
                            match crate::agent_root_service::load_agent_roots() {
                                Ok(roots) => (roots, None),
                                Err(error) => (Vec::new(), Some(error)),
                            };
                        let (registry, build_error) =
                            crate::agent_root_service::build_agent_root_registry(&dirs, &roots);
                        let error = read_error.or(build_error);
                        if error.is_some() {
                            // 错误类别已作为显式状态保留；这里只记脱敏诊断。
                            log::warn!("agent root settings unavailable; keeping builtin roots");
                        }
                        // 展示过滤与树共用同一配置快照；与根同批后台读取。
                        let config = super::settings_agents::load_agent_files_config();
                        (dirs, roots, registry, error, config)
                    });
                    cx.spawn(async move |this, cx| {
                        let (dirs, roots, registry, error, config) = loading.await;
                        let _ = this.update(cx, |root, cx| {
                            // 已注入或更早完成的快照优先，迟到回投不得覆盖。
                            if root.agent_roots_loaded {
                                return;
                            }
                            root.agent_global_dirs = Some(dirs);
                            root.custom_agent_roots =
                                if error.is_some() { Vec::new() } else { roots };
                            root.agent_root_registry = Some(registry);
                            root.agent_roots_error = error;
                            root.agent_roots_loaded = true;
                            if !root.agent_files_config_loaded {
                                root.agent_files_config_loaded = true;
                                match config {
                                    Ok(config) => {
                                        super::settings_agents::apply_agent_file_aliases(&config);
                                        root.agent_files_config = config;
                                        root.agent_files_config_error = None;
                                    }
                                    Err(error) => {
                                        root.agent_files_config_error = Some(error);
                                    }
                                }
                            }
                            root.sync_agent_files_roots_after_load(cx);
                            cx.notify();
                        });
                    })
                },
            )
            .expect("AgentRootsLoad 单任务登记必须唯一");
    }
}
