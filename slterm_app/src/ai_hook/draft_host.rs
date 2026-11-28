//! Hooks 草稿宿主：按「面板实例＋CLI＋层级」隔离的未提交编辑。
//!
//! 草稿不持久化；项目移除与根重定位前必须经 [`HooksDraftHost::project_has_drafts`]
//! 守卫，User 层草稿不绑定项目。草稿模型与解析归 [`super::draft`]。

use serde_json::Value;
use slterm_agents::AgentKind;
use slterm_workspace::{PanelId, ProjectId};

use super::draft::{DraftParseError, parse_hooks_json};
use super::editor::{HooksEditError, validate_editable_all};

/// Claude 编辑器可用的三层；Project／Local 绑定 Settings 所属项目根。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HooksDraftLayer {
    User,
    Project(ProjectId),
    Local(ProjectId),
}

impl HooksDraftLayer {
    /// 层级绑定的项目；User 层不随项目存亡变化。
    pub(crate) fn project(self) -> Option<ProjectId> {
        match self {
            Self::User => None,
            Self::Project(project) | Self::Local(project) => Some(project),
        }
    }
}

/// 草稿所有者：项目设置面板或应用级设置浮层。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HooksDraftOwner {
    Panel(PanelId),
    Overlay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HooksDraftKey {
    pub(crate) owner: HooksDraftOwner,
    pub(crate) agent: AgentKind,
    pub(crate) layer: HooksDraftLayer,
}

impl HooksDraftKey {
    pub(crate) const fn panel(self) -> Option<PanelId> {
        match self.owner {
            HooksDraftOwner::Panel(panel) => Some(panel),
            HooksDraftOwner::Overlay => None,
        }
    }
}

/// 单个草稿：原始 JSON 文本、最后合法模型、磁盘基线指纹与诊断。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HooksDraft {
    baseline: Option<[u8; 32]>,
    saved_text: String,
    text: String,
    model: Value,
    diagnostic: Option<DraftParseError>,
    /// 最后合法模型的 schema 诊断；JSON 非法期间保留上一模型的事实。
    schema_errors: Vec<HooksEditError>,
}

impl HooksDraft {
    /// 从已加载文件进入编辑：`text` 必须是调用方确认可解析的 hooks 子树文本。
    pub(crate) fn loaded(baseline: Option<[u8; 32]>, text: String, model: Value) -> Self {
        let schema_errors = validate_editable_all(&model);
        Self {
            baseline,
            saved_text: text.clone(),
            text,
            model,
            diagnostic: None,
            schema_errors,
        }
    }

    /// 更新 JSON 文本：解析失败保留最后合法模型并记录首条诊断，禁用保存。
    pub(crate) fn set_text(&mut self, text: &str) {
        self.text = text.to_owned();
        match parse_hooks_json(text) {
            Ok(model) => {
                self.schema_errors = validate_editable_all(&model);
                self.model = model;
                self.diagnostic = None;
            }
            Err(error) => self.diagnostic = Some(error),
        }
    }

    /// 保存成功回执结算：只有提交文本仍是当前文本时才替换为磁盘规范文本并回到干净态；
    /// 保存期间的新编辑保留正文、模型与诊断，只推进基线，避免迟到回执覆盖新输入。
    /// 返回是否替换了正文（调用方据此决定是否同步编辑区）。
    pub(crate) fn finish_save(
        &mut self,
        submitted: &str,
        baseline: [u8; 32],
        saved_text: String,
        saved_model: Value,
    ) -> bool {
        self.baseline = Some(baseline);
        if self.text != submitted {
            self.saved_text = saved_text;
            return false;
        }
        self.saved_text = saved_text.clone();
        self.text = saved_text;
        self.schema_errors = validate_editable_all(&saved_model);
        self.model = saved_model;
        self.diagnostic = None;
        true
    }

    pub(crate) fn baseline(&self) -> Option<[u8; 32]> {
        self.baseline
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// 最后一次合法模型；非法 JSON 期间仍是旧模型，GUI 编辑随之禁用。
    pub(crate) fn model(&self) -> &Value {
        &self.model
    }

    pub(crate) fn diagnostic(&self) -> Option<&DraftParseError> {
        self.diagnostic.as_ref()
    }

    /// 最后合法模型的 schema 诊断（未知事件、非法 handler 等）。
    pub(crate) fn schema_errors(&self) -> &[HooksEditError] {
        &self.schema_errors
    }

    /// 非法 JSON 或尚未保存的文本都视为脏；两者都需要保存／丢弃守卫。
    pub(crate) fn dirty(&self) -> bool {
        self.diagnostic.is_some() || self.text != self.saved_text
    }

    /// 非法 JSON／schema 均禁用保存；没有改动时也没有可保存内容。
    pub(crate) fn saveable(&self) -> bool {
        self.diagnostic.is_none() && self.schema_errors.is_empty() && self.dirty()
    }
}

/// 面板实例唯一持有的草稿表；插入顺序即打开顺序，规模远小于线性扫描成本。
#[derive(Debug, Default)]
pub(crate) struct HooksDraftHost {
    drafts: Vec<(HooksDraftKey, HooksDraft)>,
}

impl HooksDraftHost {
    pub(crate) fn get(&self, key: HooksDraftKey) -> Option<&HooksDraft> {
        self.drafts
            .iter()
            .find(|(existing, _)| *existing == key)
            .map(|(_, draft)| draft)
    }

    pub(crate) fn get_mut(&mut self, key: HooksDraftKey) -> Option<&mut HooksDraft> {
        self.drafts
            .iter_mut()
            .find(|(existing, _)| *existing == key)
            .map(|(_, draft)| draft)
    }

    /// 同一键再次打开（切页／切层回来）复用既有草稿；不同键互不影响。
    pub(crate) fn insert(&mut self, key: HooksDraftKey, draft: HooksDraft) {
        if let Some((_, existing)) = self
            .drafts
            .iter_mut()
            .find(|(existing, _)| *existing == key)
        {
            *existing = draft;
        } else {
            self.drafts.push((key, draft));
        }
    }

    pub(crate) fn discard(&mut self, key: HooksDraftKey) -> Option<HooksDraft> {
        let index = self
            .drafts
            .iter()
            .position(|(existing, _)| *existing == key)?;
        Some(self.drafts.remove(index).1)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.drafts.is_empty()
    }

    pub(crate) fn keys(&self) -> Vec<HooksDraftKey> {
        self.drafts.iter().map(|(key, _)| *key).collect()
    }

    pub(crate) fn len(&self) -> usize {
        self.drafts.len()
    }

    /// 任一层级存在未提交草稿即需守卫；含非法 JSON 与未保存文本。
    pub(crate) fn dirty_drafts(&self) -> impl Iterator<Item = (HooksDraftKey, &HooksDraft)> {
        self.drafts
            .iter()
            .filter(|(_, draft)| draft.dirty())
            .map(|(key, draft)| (*key, draft))
    }

    /// 项目移除与根重定位守卫：绑定该项目且存在未提交编辑的草稿才拒绝；
    /// 与磁盘一致的干净草稿不承载用户数据，不得阻塞项目操作。
    pub(crate) fn project_has_drafts(&self, project: ProjectId) -> bool {
        self.drafts
            .iter()
            .any(|(key, draft)| key.layer.project() == Some(project) && draft.dirty())
    }

    /// 该所有者是否仍有未提交草稿。
    pub(crate) fn owner_has_dirty(&self, owner: HooksDraftOwner) -> bool {
        self.drafts
            .iter()
            .any(|(key, draft)| key.owner == owner && draft.dirty())
    }

    /// 所有者销毁时释放其全部层级草稿。
    pub(crate) fn clear_owner(&mut self, owner: HooksDraftOwner) -> Vec<HooksDraftKey> {
        let mut cleared = Vec::new();
        self.drafts.retain(|(key, _)| {
            if key.owner == owner {
                cleared.push(*key);
                false
            } else {
                true
            }
        });
        cleared
    }

    /// 面板销毁时释放该面板的全部层级草稿，避免键指向已销毁的 PanelId。
    pub(crate) fn clear_panel(&mut self, panel: PanelId) -> Vec<HooksDraftKey> {
        self.clear_owner(HooksDraftOwner::Panel(panel))
    }

    /// 项目专属草稿随项目移除清理；User 草稿保留，不因项目存亡丢失。
    pub(crate) fn clear_project(&mut self, project: ProjectId) -> Vec<HooksDraftKey> {
        let mut cleared = Vec::new();
        self.drafts.retain(|(key, _)| {
            if key.layer.project() == Some(project) {
                cleared.push(*key);
                false
            } else {
                true
            }
        });
        cleared
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slterm_workspace::PanelIdAllocator;

    fn panel(epoch: u64) -> PanelId {
        PanelIdAllocator::new(epoch).allocate().unwrap()
    }

    fn project(text: &str) -> ProjectId {
        use std::str::FromStr as _;
        ProjectId::from_str(text).unwrap()
    }

    const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";

    fn draft(text: &str) -> HooksDraft {
        let model = parse_hooks_json(text).unwrap();
        HooksDraft::loaded(None, text.to_owned(), model)
    }

    /// 三层与面板实例相互隔离；同键重开复用同一草稿，不覆盖未提交文本。
    #[test]
    fn hooks_drafts_are_isolated_per_panel_agent_and_layer() {
        let mut host = HooksDraftHost::default();
        let key_user = HooksDraftKey {
            owner: HooksDraftOwner::Panel(panel(1)),
            agent: AgentKind::Claude,
            layer: HooksDraftLayer::User,
        };
        let key_project = HooksDraftKey {
            layer: HooksDraftLayer::Project(project(PROJECT_A)),
            ..key_user
        };
        let key_local = HooksDraftKey {
            layer: HooksDraftLayer::Local(project(PROJECT_A)),
            ..key_user
        };
        let key_other_panel = HooksDraftKey {
            owner: HooksDraftOwner::Panel(panel(2)),
            ..key_user
        };
        host.insert(key_user, draft(r#"{"Stop":[]}"#));
        host.insert(key_project, draft(r#"{"Prompt":[]}"#));
        host.insert(key_local, draft(r#"{"Local":[]}"#));
        host.insert(key_other_panel, draft(r#"{"Panel":[]}"#));
        assert_eq!(host.len(), 4);

        host.get_mut(key_local)
            .unwrap()
            .set_text(r#"{"Local":[{"hooks":[{"type":"command","command":"x"}]}]}"#);
        assert!(host.get(key_local).unwrap().dirty());
        assert!(!host.get(key_user).unwrap().dirty());
        assert!(!host.get(key_project).unwrap().dirty());
        assert!(!host.get(key_other_panel).unwrap().dirty());

        let reopened = host.get(key_user).map(|draft| draft.text().to_owned());
        assert_eq!(reopened.as_deref(), Some(r#"{"Stop":[]}"#));
    }

    /// 保存回执结算：提交后未再编辑才采用磁盘规范文本；期间的新编辑保留正文与脏态。
    #[test]
    fn save_receipt_keeps_edits_made_while_the_save_was_in_flight() {
        let mut draft = draft(r#"{"Stop":[]}"#);
        let submitted = draft.text().to_owned();
        draft.set_text(r#"{"Stop":[{"hooks":[{"type":"command","command":"newer"}]}]}"#);
        let edited = draft.text().to_owned();
        let applied = draft.finish_save(
            &submitted,
            [7; 32],
            "{\n  \"Stop\": []\n}".to_owned(),
            serde_json::json!({"Stop": []}),
        );
        assert!(!applied, "迟到回执不得替换保存期间的新编辑");
        assert_eq!(draft.text(), edited);
        assert!(draft.dirty(), "新编辑必须保持脏态以便再次保存");
        assert_eq!(
            draft.baseline(),
            Some([7; 32]),
            "基线必须前移到刚写入的版本"
        );

        let submitted = draft.text().to_owned();
        let applied = draft.finish_save(
            &submitted,
            [9; 32],
            "{\n  \"Stop\": []\n}".to_owned(),
            serde_json::json!({"Stop": []}),
        );
        assert!(applied, "正文未变时必须采用磁盘规范文本");
        assert!(!draft.dirty());
        assert_eq!(draft.text(), "{\n  \"Stop\": []\n}");
    }

    /// 面板销毁只释放该面板的草稿，其他面板与 User 层不受影响。
    #[test]
    fn clear_panel_removes_only_that_panel_drafts() {
        let mut host = HooksDraftHost::default();
        let closing = panel(11);
        let survivor = panel(12);
        let closing_key = HooksDraftKey {
            owner: HooksDraftOwner::Panel(closing),
            agent: AgentKind::Claude,
            layer: HooksDraftLayer::Project(project(PROJECT_A)),
        };
        let survivor_key = HooksDraftKey {
            owner: HooksDraftOwner::Panel(survivor),
            ..closing_key
        };
        host.insert(closing_key, draft(r#"{"Stop":[]}"#));
        host.insert(survivor_key, draft(r#"{"Stop":[]}"#));
        let cleared = host.clear_panel(closing);
        assert_eq!(cleared, vec![closing_key]);
        assert!(host.get(closing_key).is_none());
        assert!(host.get(survivor_key).is_some());
    }

    /// 非法 JSON 保留最后合法模型并禁用保存；修正后恢复可保存。
    #[test]
    fn hooks_invalid_json_keeps_last_valid_model_and_blocks_save() {
        let mut draft = draft(r#"{"Stop":[]}"#);
        draft.set_text("{broken");
        assert!(!draft.saveable(), "非法模型必须禁用保存");
        assert!(draft.dirty());
        assert_eq!(draft.model(), &serde_json::json!({"Stop": []}));
        assert!(draft.diagnostic().is_some());

        draft.set_text(r#"{"Stop":[{"hooks":[{"type":"command","command":"x"}]}]}"#);
        assert!(draft.saveable(), "修正后应可保存");
        assert!(draft.diagnostic().is_none());
    }

    /// 项目守卫只看 Project／Local；User 草稿不阻塞，清理只清项目专属草稿。
    #[test]
    fn hooks_project_guard_ignores_user_layer_and_clears_project_drafts() {
        let mut host = HooksDraftHost::default();
        let panel = panel(1);
        let project_a = project(PROJECT_A);
        let project_b = project(PROJECT_B);
        let user_key = HooksDraftKey {
            owner: HooksDraftOwner::Panel(panel),
            agent: AgentKind::Claude,
            layer: HooksDraftLayer::User,
        };
        let project_key = HooksDraftKey {
            layer: HooksDraftLayer::Project(project_a),
            ..user_key
        };
        let local_key = HooksDraftKey {
            layer: HooksDraftLayer::Local(project_a),
            ..user_key
        };
        host.insert(user_key, draft(r#"{"Stop":[]}"#));
        assert!(!host.project_has_drafts(project_a), "User 草稿不绑定项目");

        host.insert(project_key, draft(r#"{"Stop":[]}"#));
        host.insert(local_key, draft(r#"{"Stop":[]}"#));
        assert!(
            !host.project_has_drafts(project_a),
            "与磁盘一致的草稿不得阻塞项目操作"
        );
        host.get_mut(project_key)
            .unwrap()
            .set_text(r#"{"Stop":[{"hooks":[{"type":"command","command":"x"}]}]}"#);
        assert!(host.project_has_drafts(project_a), "未提交编辑必须阻塞");
        assert!(!host.project_has_drafts(project_b));

        let mut cleared = host.clear_project(project_a);
        cleared.sort_by_key(|key| key.layer.project());
        assert_eq!(cleared.len(), 2);
        assert!(!host.project_has_drafts(project_a));
        assert!(host.get(user_key).is_some(), "User 草稿不得随项目移除丢失");
    }
}
