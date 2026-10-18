//! 任务栏进度:OSC 9;4 → Windows 任务栏。
//!
//! OSC 9;4 是 ConEmu 定的序列,Windows 任务栏原生就有这套语义
//! (`ITaskbarList3::SetProgressState`/`SetProgressValue`),所以这条链路在
//! Windows 上几乎是零成本:程序在任务开始、进度变化和结束时各发一次,任务栏
//! 就有了真实进度——支持该协议的构建工具跑着,切到别的窗口也看得见。
//!
//! 状态映射刻意宽容:解析层(`slterm_terminal` 的 OSC sniffer)不收窄状态码,
//! 这里把规范外的码一律当成「清除」。部分 shell 集成实测会发 `9;4;5;0`
//! 表示成功完成,而 ConEmu 只定义了 0..=4——把未知码当非法丢掉,等于让进度条
//! 永远停在最后一个状态上。
//!
//! 路由是进程级单显示(单窗单实例):任务栏只有一条进度条,只反映活跃 tab
//! 的聚焦 pane([`TaskbarState`]);命令收尾/agent 退出/pane 关闭即清,不留残条。
//! 真实 COM 投递面按豁免登记走人工 smoke,纯映射与状态机在 L1 全覆盖。

use slterm_split::PaneId;

/// 任务栏能画出来的进度形态(ConEmu 9;4 状态码的五态模型)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TaskProgress {
    /// 没有进度,任务栏恢复常态。
    #[default]
    None,
    /// 不确定进度(来回扫的条):知道在忙,不知道还剩多少。
    Indeterminate,
    /// 正常进度,0..=100。
    Value(u8),
    /// 出错(红条)。命令失败时 shell 集成会发这个。
    Error(Option<u8>),
    /// 暂停或需要注意(黄条)。
    Paused(Option<u8>),
}

impl TaskProgress {
    /// 按 ConEmu 的状态码映射。0 清除 / 1 正常 / 2 错误 / 3 不确定 / 4 暂停,
    /// 其余(包括部分 shell 集成用来表示成功的 5)都归到清除。
    pub(crate) fn from_osc(state: u8, value: Option<u8>) -> Self {
        match state {
            1 => Self::Value(value.unwrap_or(0).min(100)),
            2 => Self::Error(value.map(|percent| percent.min(100))),
            3 => Self::Indeterminate,
            4 => Self::Paused(value.map(|percent| percent.min(100))),
            _ => Self::None,
        }
    }

    /// 任务栏是否需要画东西。
    pub(crate) fn is_active(self) -> bool {
        self != Self::None
    }

    /// `TBPFLAG` 值。
    #[cfg(windows)]
    fn taskbar_flag(self) -> i32 {
        match self {
            Self::None => 0,          // TBPF_NOPROGRESS
            Self::Indeterminate => 1, // TBPF_INDETERMINATE
            Self::Value(_) => 2,      // TBPF_NORMAL
            Self::Error(_) => 4,      // TBPF_ERROR
            Self::Paused(_) => 8,     // TBPF_PAUSED
        }
    }

    /// 有确定百分比时返回它。错误和暂停态可以带值也可以不带——不带值时只染色,
    /// 不动条的长度(这正是 `SetProgressState` 单独存在的原因)。
    #[cfg(windows)]
    fn percent(self) -> Option<u8> {
        match self {
            Self::Value(percent) => Some(percent),
            Self::Error(percent) | Self::Paused(percent) => percent,
            Self::None | Self::Indeterminate => None,
        }
    }
}

/// 任务栏进度的进程级路由状态(单窗单实例只有一条进度条)。
///
/// 只认活跃 tab 的聚焦 pane:后台 tab/pane 的构建进度投到同一条按钮上只会
/// 互相覆盖,读数还不如没有。`current` 记下当前显示属于哪个 pane,让清除边
/// (命令收尾/agent 退出、pane 关闭、OSC 0)只抹掉自己的条,别的 pane 的收尾
/// 不会打断正在显示的进度。
#[derive(Debug, Default)]
pub(crate) struct TaskbarState {
    /// 当前在任务栏上显示的进度及其来源 pane;`None` = 任务栏常态。
    current: Option<(PaneId, TaskProgress)>,
}

impl TaskbarState {
    pub(crate) const fn new() -> Self {
        Self { current: None }
    }

    /// 活跃聚焦 pane 上报的 OSC 9;4。返回真正需要投递的变化;同 pane 同形态
    /// 去重——`cargo build` 每秒发好几次进度更新,逐条过 COM 纯属浪费。
    pub(crate) fn observe(&mut self, pane: PaneId, progress: TaskProgress) -> Option<TaskProgress> {
        if !progress.is_active() {
            return self.clear_for(pane);
        }
        if self.current == Some((pane, progress)) {
            return None;
        }
        self.current = Some((pane, progress));
        Some(progress)
    }

    /// 清除边:只有当前显示的来源 pane 自己收尾才清。返回 `Some(TaskProgress::None)`
    /// 表示需要把任务栏恢复常态;别的 pane 的收尾返回 `None`,不投递。
    pub(crate) fn clear_for(&mut self, pane: PaneId) -> Option<TaskProgress> {
        if self.current.is_some_and(|(source, _)| source == pane) {
            self.current = None;
            return Some(TaskProgress::None);
        }
        None
    }
}

/// 测试构建不触真实 COM:虚拟窗口没有原生 HWND,且任务栏是进程级外部状态,
/// 不可重复。记录调用序列,供工作区清除路径断言;真实投递面照豁免
/// 走构建产物人工 smoke。
#[cfg(all(test, feature = "gpui-test-support"))]
static APPLIED: std::sync::Mutex<Vec<(isize, TaskProgress)>> = std::sync::Mutex::new(Vec::new());

#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) fn take_applied_for_test() -> Vec<(isize, TaskProgress)> {
    std::mem::take(&mut *APPLIED.lock().unwrap_or_else(|error| error.into_inner()))
}

/// 把进度画到这个窗口的任务栏按钮上。
///
/// 失败一律静默:任务栏是纯装饰,`ITaskbarList3` 在旧系统、精简版系统或者
/// explorer 刚崩过时都可能拿不到,那不该影响终端本身。
#[cfg(windows)]
pub(crate) fn apply(hwnd: isize, progress: TaskProgress) {
    use std::ffi::c_void;

    use windows_sys::Win32::Foundation::{HWND, RPC_E_CHANGED_MODE};
    use windows_sys::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
        CoInitializeEx, CoUninitialize,
    };
    use windows_sys::core::{GUID, HRESULT};

    #[cfg(all(test, feature = "gpui-test-support"))]
    APPLIED
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push((hwnd, progress));

    const CLSID_TASKBAR_LIST: GUID = GUID::from_u128(0x56fdf344_fd6d_11d0_958a_006097c9a090);
    const IID_TASKBAR_LIST3: GUID = GUID::from_u128(0xea1afb91_9e28_4b86_90e9_9e9f8a5eefaf);

    #[repr(C)]
    struct Interface<T> {
        vtable: *const T,
    }
    #[repr(C)]
    struct IUnknownVTable {
        _query_interface: usize,
        _add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
    }
    // ITaskbarList3 继承 ITaskbarList2 继承 ITaskbarList,vtable 按继承顺序
    // 平铺。只声明我们要用的三个,其余占位。
    #[repr(C)]
    struct TaskbarList3VTable {
        base: IUnknownVTable,
        hr_init: unsafe extern "system" fn(*mut c_void) -> HRESULT,
        _add_tab: usize,
        _delete_tab: usize,
        _activate_tab: usize,
        _set_active_alt: usize,
        _mark_fullscreen_window: usize,
        set_progress_value: unsafe extern "system" fn(*mut c_void, HWND, u64, u64) -> HRESULT,
        set_progress_state: unsafe extern "system" fn(*mut c_void, HWND, i32) -> HRESULT,
    }

    if hwnd == 0 {
        return;
    }
    let initialized = unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    };
    let should_uninitialize = initialized >= 0;
    if initialized < 0 && initialized != RPC_E_CHANGED_MODE {
        return;
    }
    let mut pointer: *mut c_void = std::ptr::null_mut();
    // SAFETY: 两个 GUID 是常量,`pointer` 是本帧的局部变量。
    let created = unsafe {
        CoCreateInstance(
            &CLSID_TASKBAR_LIST,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_TASKBAR_LIST3,
            &mut pointer,
        )
    };
    if created >= 0 && !pointer.is_null() {
        // SAFETY: CoCreateInstance 成功即 `pointer` 指向一个 ITaskbarList3;
        // 下面只按上面声明的 vtable 布局调用它自己的方法。
        unsafe {
            let interface = pointer as *mut Interface<TaskbarList3VTable>;
            let vtable = &*(*interface).vtable;
            // HrInit 必须先调;不调的话后续方法在部分系统上直接返回失败。
            if (vtable.hr_init)(pointer) >= 0 {
                let handle = hwnd as HWND;
                (vtable.set_progress_state)(pointer, handle, progress.taskbar_flag());
                if let Some(percent) = progress.percent() {
                    (vtable.set_progress_value)(pointer, handle, u64::from(percent), 100);
                }
            }
            (vtable.base.release)(pointer);
        }
    }
    if should_uninitialize {
        // SAFETY: 与上面成功的 CoInitializeEx 配对。
        unsafe { CoUninitialize() };
    }
}

#[cfg(not(windows))]
pub(crate) fn apply(_hwnd: isize, _progress: TaskProgress) {}

#[cfg(test)]
mod taskbar_tests {
    use super::{TaskProgress, TaskbarState};
    use slterm_split::PaneId;

    /// 规范码映射到五态;未知码(含实测 `9;4;5;0`)归清除而不是被丢弃——
    /// 否则进度条会永远停在最后一个状态上。
    #[test]
    fn known_states_map_and_unknown_states_clear() {
        assert_eq!(TaskProgress::from_osc(0, None), TaskProgress::None);
        assert_eq!(TaskProgress::from_osc(1, Some(42)), TaskProgress::Value(42));
        assert_eq!(TaskProgress::from_osc(2, None), TaskProgress::Error(None));
        assert_eq!(
            TaskProgress::from_osc(2, Some(80)),
            TaskProgress::Error(Some(80))
        );
        assert_eq!(TaskProgress::from_osc(3, None), TaskProgress::Indeterminate);
        assert_eq!(
            TaskProgress::from_osc(4, Some(90)),
            TaskProgress::Paused(Some(90))
        );

        assert_eq!(TaskProgress::from_osc(5, Some(0)), TaskProgress::None);
        assert_eq!(TaskProgress::from_osc(200, None), TaskProgress::None);
        assert!(!TaskProgress::from_osc(5, Some(0)).is_active());
        assert_eq!(TaskProgress::from_osc(1, None), TaskProgress::Value(0));
    }

    /// 百分比越界要收进 0..=100:`SetProgressValue` 的分母写死 100,传进去的
    /// completed 比 total 大会画出一条撑满且不动的条。
    #[test]
    fn percentages_are_clamped() {
        assert_eq!(
            TaskProgress::from_osc(1, Some(200)),
            TaskProgress::Value(100)
        );
        assert_eq!(
            TaskProgress::from_osc(2, Some(255)),
            TaskProgress::Error(Some(100))
        );
        assert_eq!(
            TaskProgress::from_osc(4, Some(255)),
            TaskProgress::Paused(Some(100))
        );
    }

    /// `TBPFLAG` 与百分比:错误/暂停可只染色不带值;不确定态无百分比。
    #[cfg(windows)]
    #[test]
    fn taskbar_flags_and_percentages_match_the_five_states() {
        assert_eq!(TaskProgress::None.taskbar_flag(), 0);
        assert_eq!(TaskProgress::Indeterminate.taskbar_flag(), 1);
        assert_eq!(TaskProgress::Value(42).taskbar_flag(), 2);
        assert_eq!(TaskProgress::Error(None).taskbar_flag(), 4);
        assert_eq!(TaskProgress::Paused(None).taskbar_flag(), 8);

        assert_eq!(TaskProgress::Value(42).percent(), Some(42));
        assert_eq!(TaskProgress::Error(None).percent(), None);
        assert_eq!(TaskProgress::Error(Some(7)).percent(), Some(7));
        assert_eq!(TaskProgress::Paused(Some(9)).percent(), Some(9));
        assert_eq!(TaskProgress::None.percent(), None);
        assert_eq!(TaskProgress::Indeterminate.percent(), None);
    }

    /// 同 pane 同形态去重;换 pane 或换形态才再投递。
    #[test]
    fn routing_state_dedupes_identical_updates() {
        let mut state = TaskbarState::new();
        assert_eq!(
            state.observe(PaneId(1), TaskProgress::Value(42)),
            Some(TaskProgress::Value(42))
        );
        assert_eq!(state.observe(PaneId(1), TaskProgress::Value(42)), None);
        assert_eq!(
            state.observe(PaneId(1), TaskProgress::Value(43)),
            Some(TaskProgress::Value(43))
        );
        assert_eq!(
            state.observe(PaneId(2), TaskProgress::Value(43)),
            Some(TaskProgress::Value(43)),
            "聚焦 pane 换人时,同读数也要重新投递归属"
        );
    }

    /// 清除边只对来源 pane 生效:别的 pane 的命令收尾不得抹掉正在显示的进度;
    /// 来源 pane 的 OSC 0/未知码即恢复常态。
    #[test]
    fn clear_edges_only_touch_their_own_source_pane() {
        let mut state = TaskbarState::new();
        state.observe(PaneId(1), TaskProgress::Indeterminate);
        assert_eq!(state.clear_for(PaneId(2)), None);
        assert_eq!(state.clear_for(PaneId(1)), Some(TaskProgress::None));
        assert_eq!(state.clear_for(PaneId(1)), None, "重复清除不重复投递");

        state.observe(PaneId(1), TaskProgress::Value(30));
        assert_eq!(
            state.observe(PaneId(1), TaskProgress::None),
            Some(TaskProgress::None)
        );
        state.observe(PaneId(1), TaskProgress::Value(30));
        assert_eq!(
            state.observe(PaneId(1), TaskProgress::from_osc(5, Some(0))),
            Some(TaskProgress::None)
        );
    }
}
