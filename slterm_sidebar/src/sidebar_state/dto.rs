//! 侧栏组织状态的严格规范 DTO。

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde_json::Value;
use slterm_workspace::SideViewId;

use super::{SideSlot, SidebarState};

pub(super) const SIDEBAR_STATE_VERSION: u64 = 1;
pub(super) const SIDEBAR_STATE_KIND: &str = "sidebar_state";

/// 侧栏组织状态的编码或严格解码错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarDtoError {
    /// JSON、版本信封或字段集合不符合当前结构。
    InvalidStructure,
    /// 外层类型名不是侧栏组织状态。
    UnknownKind,
    /// DTO 版本不是当前版本。
    UnsupportedVersion,
    /// 持久文本包含未登记的侧栏视图。
    UnknownView,
    /// 按钮、槽位或几何值违反领域不变量。
    InvalidState,
    /// 规范 JSON 编码失败。
    Encode,
}

impl fmt::Display for SidebarDtoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidStructure => "侧栏组织状态持久结构非法",
            Self::UnknownKind => "未知侧栏组织状态类型",
            Self::UnsupportedVersion => "不支持的侧栏组织状态 DTO 版本",
            Self::UnknownView => "侧栏组织状态包含未知视图",
            Self::InvalidState => "侧栏组织状态违反领域不变量",
            Self::Encode => "侧栏组织状态规范 JSON 编码失败",
        })
    }
}

impl std::error::Error for SidebarDtoError {}

impl SidebarState {
    /// 从独立版本的规范 JSON 严格恢复侧栏组织状态。
    pub fn from_canonical_json(text: &str) -> Result<Self, SidebarDtoError> {
        let envelope: SidebarStateEnvelope =
            serde_json::from_str(text).map_err(|_| SidebarDtoError::InvalidStructure)?;
        if envelope.version != SIDEBAR_STATE_VERSION {
            return Err(SidebarDtoError::UnsupportedVersion);
        }
        if envelope.kind != SIDEBAR_STATE_KIND {
            return Err(SidebarDtoError::UnknownKind);
        }
        let dto: SidebarStateDto =
            serde_json::from_str(text).map_err(|_| SidebarDtoError::InvalidStructure)?;
        dto.into_domain()
    }

    /// 序列化为独立版本、确定键序且可逐字节重放的 JSON。
    pub fn to_canonical_json(&self) -> Result<String, SidebarDtoError> {
        let mut buttons = Vec::new();
        let mut open_views = Vec::new();
        for slot in SideSlot::ALL {
            buttons.extend(
                self.buttons(slot)
                    .iter()
                    .map(|view| view_record_value(slot, *view)),
            );
            if let Some(view) = self.open_view(slot) {
                open_views.push(view_record_value(slot, view));
            }
        }
        let hidden_buttons = Value::Array(
            SideViewId::ALL
                .into_iter()
                .filter(|view| self.hidden_buttons.contains(view))
                .map(|view| Value::String(view.id_str().to_owned()))
                .collect(),
        );
        let object = BTreeMap::from([
            ("buttons", Value::Array(buttons)),
            ("hidden_buttons", hidden_buttons),
            ("kind", Value::String(SIDEBAR_STATE_KIND.to_owned())),
            ("open_views", Value::Array(open_views)),
            ("ratio", Value::from(self.ratio)),
            ("version", Value::from(SIDEBAR_STATE_VERSION)),
            ("visible_intent", Value::from(self.visible_intent)),
            ("width", Value::from(self.width)),
        ]);
        serde_json::to_string(&object_value(object)).map_err(|_| SidebarDtoError::Encode)
    }
}

#[derive(Deserialize)]
struct SidebarStateEnvelope {
    version: u64,
    kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SidebarStateDto {
    #[serde(rename = "version")]
    _version: u64,
    #[serde(rename = "kind")]
    _kind: String,
    buttons: Vec<ViewRecordDto>,
    hidden_buttons: Vec<String>,
    open_views: Vec<ViewRecordDto>,
    visible_intent: bool,
    width: u16,
    ratio: f32,
}

impl SidebarStateDto {
    fn into_domain(self) -> Result<SidebarState, SidebarDtoError> {
        let mut buttons: [Vec<SideViewId>; 2] = Default::default();
        for record in self.buttons {
            let (slot, view) = record.into_domain()?;
            buttons[slot.index()].push(view);
        }
        let hidden_buttons = parse_views(self.hidden_buttons)?;
        let mut open_views = [None, None];
        for record in self.open_views {
            let (slot, view) = record.into_domain()?;
            if open_views[slot.index()].replace(view).is_some() {
                return Err(SidebarDtoError::InvalidState);
            }
        }
        SidebarState::from_persisted_parts(
            buttons,
            hidden_buttons,
            open_views,
            self.visible_intent,
            self.width,
            self.ratio,
        )
        .ok_or(SidebarDtoError::InvalidState)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewRecordDto {
    slot: String,
    view: String,
}

impl ViewRecordDto {
    fn into_domain(self) -> Result<(SideSlot, SideViewId), SidebarDtoError> {
        let slot = SideSlot::from_id_str(&self.slot).ok_or(SidebarDtoError::InvalidStructure)?;
        let view = self
            .view
            .parse()
            .map_err(|_| SidebarDtoError::UnknownView)?;
        Ok((slot, view))
    }
}

fn parse_views(texts: Vec<String>) -> Result<Vec<SideViewId>, SidebarDtoError> {
    texts
        .into_iter()
        .map(|text| text.parse().map_err(|_| SidebarDtoError::UnknownView))
        .collect()
}

fn view_record_value(slot: SideSlot, view: SideViewId) -> Value {
    object_value(BTreeMap::from([
        ("slot", Value::String(slot.id_str().to_owned())),
        ("view", Value::String(view.id_str().to_owned())),
    ]))
}

fn object_value(object: BTreeMap<&'static str, Value>) -> Value {
    Value::Object(
        object
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
