//! 侧栏组织状态的容错恢复与结构损坏分流。

use std::collections::BTreeSet;
use std::fmt;

use serde::de::{DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, de};
use serde_json::value::RawValue;
use slterm_workspace::SideViewId;

use super::dto::{SIDEBAR_STATE_KIND, SIDEBAR_STATE_VERSION};
use super::{SideSlot, SidebarState};

type RawField = Option<Box<RawValue>>;
type RawRecords = Vec<Box<RawValue>>;

/// 恢复期间可聚合报告的字段修复与归一类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SidebarRecoveryWarning {
    /// 未登记的视图记录已过滤。
    UnknownView,
    /// 重复视图只保留首条有效记录。
    DuplicateView,
    /// 同一实际槽位的多个打开请求只保留首个。
    ConflictingOpenView,
    /// 按钮字段或记录非法，已回退或按默认槽位修复。
    InvalidButtons,
    /// 隐藏按钮字段或元素非法，已回退或过滤。
    InvalidHiddenButtons,
    /// 打开视图字段、记录或请求槽位非法，已回退或归一。
    InvalidOpenViews,
    /// 显示意图字段非法，已使用默认值。
    InvalidVisibleIntent,
    /// 宽度缺失、类型错误或越界，已使用默认值。
    InvalidWidth,
    /// 比例缺失、类型错误、非有限或越界，已使用默认值。
    InvalidRatio,
}

/// 无法进入字段级恢复的持久输入错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarRecoveryError {
    /// JSON、重复字段或顶层字段集合不符合当前结构。
    InvalidStructure,
    /// 外层类型名不是侧栏组织状态。
    UnknownKind,
    /// DTO 版本不是当前版本。
    UnsupportedVersion,
}

impl fmt::Display for SidebarRecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidStructure => "侧栏恢复输入结构损坏",
            Self::UnknownKind => "未知侧栏恢复类型",
            Self::UnsupportedVersion => "不支持的侧栏恢复 DTO 版本",
        })
    }
}

impl std::error::Error for SidebarRecoveryError {}

/// 容错恢复后的有效状态与聚合警告。
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarRecovery {
    /// 满足全部领域不变量的恢复状态。
    pub state: SidebarState,
    /// 按警告类型声明顺序排列的聚合修复报告。
    pub warnings: Vec<SidebarRecoveryWarning>,
}

impl SidebarRecovery {
    /// 恢复当前版本输入；整体损坏与字段级非法分别分流。
    pub fn from_json(text: &str) -> Result<Self, SidebarRecoveryError> {
        let envelope: RecoveryEnvelope =
            serde_json::from_str(text).map_err(|_| SidebarRecoveryError::InvalidStructure)?;
        if envelope.version != SIDEBAR_STATE_VERSION {
            return Err(SidebarRecoveryError::UnsupportedVersion);
        }
        if envelope.kind != SIDEBAR_STATE_KIND {
            return Err(SidebarRecoveryError::UnknownKind);
        }

        // 当前版本先拒绝任意层级重复字段，再执行字段级容错。
        serde_json::from_str::<UniqueJson>(text)
            .map_err(|_| SidebarRecoveryError::InvalidStructure)?;
        let dto: RecoveryDto =
            serde_json::from_str(text).map_err(|_| SidebarRecoveryError::InvalidStructure)?;
        let defaults = SidebarState::default();
        let mut warnings = BTreeSet::new();
        let buttons = recover_buttons(dto.buttons, &mut warnings);
        let hidden_buttons = recover_hidden_buttons(dto.hidden_buttons, &defaults, &mut warnings);
        let open_views = recover_open_views(dto.open_views, &buttons, &defaults, &mut warnings);
        let visible_intent = recover_field(
            dto.visible_intent,
            defaults.visible_intent(),
            SidebarRecoveryWarning::InvalidVisibleIntent,
            &mut warnings,
            |_| true,
        );
        let width = recover_field(
            dto.width,
            defaults.width(),
            SidebarRecoveryWarning::InvalidWidth,
            &mut warnings,
            |width| (SidebarState::MIN_WIDTH..=SidebarState::MAX_WIDTH).contains(width),
        );
        let ratio = recover_field(
            dto.ratio,
            defaults.ratio(),
            SidebarRecoveryWarning::InvalidRatio,
            &mut warnings,
            |ratio: &f32| {
                ratio.is_finite()
                    && (SidebarState::MIN_RATIO..=SidebarState::MAX_RATIO).contains(ratio)
            },
        );
        let state = SidebarState::from_persisted_parts(
            buttons,
            hidden_buttons,
            open_views,
            visible_intent,
            width,
            ratio,
        )
        .ok_or(SidebarRecoveryError::InvalidStructure)?;
        Ok(Self {
            state,
            warnings: warnings.into_iter().collect(),
        })
    }
}

#[derive(Deserialize)]
struct RecoveryEnvelope {
    version: u64,
    kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryDto {
    #[serde(rename = "version")]
    _version: u64,
    #[serde(rename = "kind")]
    _kind: String,
    buttons: RawField,
    hidden_buttons: RawField,
    open_views: RawField,
    visible_intent: RawField,
    width: RawField,
    ratio: RawField,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryViewRecord {
    slot: String,
    view: String,
}

fn recover_field<T>(
    raw: RawField,
    fallback: T,
    warning: SidebarRecoveryWarning,
    warnings: &mut BTreeSet<SidebarRecoveryWarning>,
    is_valid: impl FnOnce(&T) -> bool,
) -> T
where
    T: DeserializeOwned,
{
    match decode_field(raw).filter(is_valid) {
        Some(value) => value,
        None => {
            warnings.insert(warning);
            fallback
        }
    }
}

fn decode_field<T: DeserializeOwned>(raw: RawField) -> Option<T> {
    serde_json::from_str(raw?.get()).ok()
}

fn decode_record(raw: &RawValue) -> Option<RecoveryViewRecord> {
    serde_json::from_str(raw.get()).ok()
}

fn recover_buttons(
    raw: RawField,
    warnings: &mut BTreeSet<SidebarRecoveryWarning>,
) -> [Vec<SideViewId>; 2] {
    let Some(records) = decode_field::<RawRecords>(raw) else {
        warnings.insert(SidebarRecoveryWarning::InvalidButtons);
        return SidebarState::default_buttons();
    };
    let mut buttons: [Vec<SideViewId>; 2] = Default::default();
    let mut registered = BTreeSet::new();
    for raw in records {
        let Some(record) = decode_record(&raw) else {
            warnings.insert(SidebarRecoveryWarning::InvalidButtons);
            continue;
        };
        let Ok(view) = record.view.parse() else {
            warnings.insert(SidebarRecoveryWarning::UnknownView);
            continue;
        };
        if !registered.insert(view) {
            warnings.insert(SidebarRecoveryWarning::DuplicateView);
            continue;
        }
        let slot = match SideSlot::from_id_str(&record.slot) {
            Some(slot) => slot,
            None => {
                warnings.insert(SidebarRecoveryWarning::InvalidButtons);
                SidebarState::default_button_slot(view)
            }
        };
        buttons[slot.index()].push(view);
    }

    // 新增注册视图是正常演进，按默认槽位和注册顺序补齐，不报告损坏。
    for view in SideViewId::ALL {
        if !registered.contains(&view) {
            buttons[SidebarState::default_button_slot(view).index()].push(view);
        }
    }
    buttons
}

fn recover_hidden_buttons(
    raw: RawField,
    defaults: &SidebarState,
    warnings: &mut BTreeSet<SidebarRecoveryWarning>,
) -> Vec<SideViewId> {
    let Some(raw_views) = decode_field::<RawRecords>(raw) else {
        warnings.insert(SidebarRecoveryWarning::InvalidHiddenButtons);
        return SideViewId::ALL
            .into_iter()
            .filter(|view| !defaults.button_is_visible(*view))
            .collect();
    };
    let mut hidden = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in raw_views {
        let Some(text) = decode_field::<String>(Some(raw)) else {
            warnings.insert(SidebarRecoveryWarning::InvalidHiddenButtons);
            continue;
        };
        let Ok(view) = text.parse() else {
            warnings.insert(SidebarRecoveryWarning::UnknownView);
            continue;
        };
        if !seen.insert(view) {
            warnings.insert(SidebarRecoveryWarning::DuplicateView);
            continue;
        }
        hidden.push(view);
    }
    hidden
}

fn recover_open_views(
    raw: RawField,
    buttons: &[Vec<SideViewId>; 2],
    defaults: &SidebarState,
    warnings: &mut BTreeSet<SidebarRecoveryWarning>,
) -> [Option<SideViewId>; 2] {
    let Some(records) = decode_field::<RawRecords>(raw) else {
        warnings.insert(SidebarRecoveryWarning::InvalidOpenViews);
        let mut open_views = [None, None];
        for slot in SideSlot::ALL {
            if let Some(view) = defaults.open_view(slot) {
                let slot = normalized_button_slot(buttons, view);
                insert_open_view(&mut open_views, slot, view, warnings);
            }
        }
        return open_views;
    };
    let mut open_views = [None, None];
    let mut seen = BTreeSet::new();
    for raw in records {
        let Some(record) = decode_record(&raw) else {
            warnings.insert(SidebarRecoveryWarning::InvalidOpenViews);
            continue;
        };
        let Ok(view) = record.view.parse() else {
            warnings.insert(SidebarRecoveryWarning::UnknownView);
            continue;
        };
        if !seen.insert(view) {
            warnings.insert(SidebarRecoveryWarning::DuplicateView);
            continue;
        }
        let requested_slot = SideSlot::from_id_str(&record.slot);
        let actual_slot = normalized_button_slot(buttons, view);
        if requested_slot != Some(actual_slot) {
            warnings.insert(SidebarRecoveryWarning::InvalidOpenViews);
        }
        insert_open_view(&mut open_views, actual_slot, view, warnings);
    }
    open_views
}

fn insert_open_view(
    open_views: &mut [Option<SideViewId>; 2],
    slot: SideSlot,
    view: SideViewId,
    warnings: &mut BTreeSet<SidebarRecoveryWarning>,
) {
    if open_views[slot.index()].is_some() {
        warnings.insert(SidebarRecoveryWarning::ConflictingOpenView);
    } else {
        open_views[slot.index()] = Some(view);
    }
}

fn normalized_button_slot(buttons: &[Vec<SideViewId>; 2], view: SideViewId) -> SideSlot {
    SideSlot::ALL
        .into_iter()
        .find(|slot| buttons[slot.index()].contains(&view))
        .expect("恢复按钮必须包含完整注册视图")
}

/// 只验证 JSON 结构与字段唯一性，不保留不可信内容。
struct UniqueJson;

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("字段唯一的 JSON")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<UniqueJson>()?.is_some() {}
        Ok(UniqueJson)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("JSON 字段重复"));
            }
            map.next_value::<UniqueJson>()?;
        }
        Ok(UniqueJson)
    }
}
