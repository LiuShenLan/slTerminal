use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::ptr;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS,
};
use windows_sys::Win32::Globalization::{
    CSTR_EQUAL, CSTR_GREATER_THAN, CSTR_LESS_THAN, CompareStringOrdinal,
};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_SZ, RegCloseKey,
    RegEnumValueW, RegOpenKeyExW, RegQueryInfoKeyW,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;

use crate::tty::Options;

const SYSTEM_ENVIRONMENT_KEY: &str =
    r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
const USER_ENVIRONMENT_KEY: &str = r"Environment";
const VOLATILE_ENVIRONMENT_KEY: &str = r"Volatile Environment";
const PATH_VARIABLES: [&str; 3] = ["Path", "LibPath", "Os2LibPath"];

static ENVIRONMENT_STATE: OnceLock<Mutex<EnvironmentState>> = OnceLock::new();

/// 为新建的 Windows 终端重建子进程环境。
///
/// `options.env` 里的现有条目是 pane 专属覆盖，最后叠加到最新注册表快照上。
pub fn refresh_environment(options: &mut Options) -> io::Result<()> {
    let state = ENVIRONMENT_STATE
        .get_or_init(|| Mutex::new(EnvironmentState::new(Environment::from_process())));
    let mut state = state
        .lock()
        .map_err(|_| io::Error::other("Windows environment refresh state is poisoned"))?;
    let snapshot = RegistrySnapshot::read()?;

    // 只有在注册表读取完整成功后才取走原覆盖项；失败时 pane 仍可沿用旧的继承模式。
    let overrides = std::mem::take(&mut options.env);
    options.env = state.merge(snapshot, overrides);
    options.env_is_complete = true;
    Ok(())
}

/// Windows 子进程环境：名称按系统规则大小写不敏感，名称和值均保留原始 UTF-16。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Environment {
    entries: BTreeMap<EnvironmentName, EnvironmentEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EnvironmentEntry {
    name: OsString,
    value: OsString,
}

#[derive(Clone, Debug)]
struct EnvironmentName(Vec<u16>);

impl EnvironmentName {
    fn new(name: impl AsRef<OsStr>) -> Self {
        Self(name.as_ref().encode_wide().collect())
    }
}

impl PartialEq for EnvironmentName {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for EnvironmentName {}

impl PartialOrd for EnvironmentName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EnvironmentName {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_environment_names(&self.0, &other.0)
    }
}

impl Environment {
    fn from_process() -> Self {
        let mut environment = Self::default();
        for (name, value) in std::env::vars_os() {
            environment.insert(name, value);
        }
        environment
    }

    pub fn insert(
        &mut self,
        name: impl AsRef<OsStr>,
        value: impl AsRef<OsStr>,
    ) -> Option<OsString> {
        let name = name.as_ref().to_os_string();
        let value = value.as_ref().to_os_string();
        self.entries
            .insert(
                EnvironmentName::new(&name),
                EnvironmentEntry { name, value },
            )
            .map(|previous| previous.value)
    }

    fn append_path(&mut self, name: OsString, value: OsString) {
        if value.is_empty() {
            return;
        }
        let normalized = EnvironmentName::new(&name);
        if let Some(existing) = self.entries.get_mut(&normalized) {
            if !existing.value.is_empty()
                && existing.value.encode_wide().last() != Some(u16::from(b';'))
            {
                existing.value.push(";");
            }
            existing.value.push(value);
        } else {
            self.entries
                .insert(normalized, EnvironmentEntry { name, value });
        }
    }

    pub fn get(&self, name: impl AsRef<OsStr>) -> Option<&OsStr> {
        self.entries
            .get(&EnvironmentName::new(name))
            .map(|entry| entry.value.as_os_str())
    }

    pub fn remove(&mut self, name: impl AsRef<OsStr>) -> Option<OsString> {
        self.entries
            .remove(&EnvironmentName::new(name))
            .map(|removed| removed.value)
    }

    fn remove_normalized(&mut self, name: &EnvironmentName) {
        self.entries.remove(name);
    }

    pub fn contains_key(&self, name: impl AsRef<OsStr>) -> bool {
        self.entries.contains_key(&EnvironmentName::new(name))
    }

    pub fn keys(&self) -> impl Iterator<Item = &OsStr> {
        self.entries.values().map(|entry| entry.name.as_os_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.entries
            .values()
            .map(|entry| (entry.name.as_os_str(), entry.value.as_os_str()))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub(super) fn compare_environment_names(left: &[u16], right: &[u16]) -> Ordering {
    let result = unsafe {
        CompareStringOrdinal(
            left.as_ptr(),
            left.len() as i32,
            right.as_ptr(),
            right.len() as i32,
            1,
        )
    };
    match result {
        CSTR_LESS_THAN => Ordering::Less,
        CSTR_EQUAL => Ordering::Equal,
        CSTR_GREATER_THAN => Ordering::Greater,
        // 环境变量名受 CreateProcess 环境块上限约束，正常不会失败；异常时仍给出
        // 全序，不能让排序过程 panic。
        _ => left.cmp(right),
    }
}

struct EnvironmentState {
    inherited: Environment,
    registry_keys: BTreeSet<EnvironmentName>,
}

impl EnvironmentState {
    fn new(inherited: Environment) -> Self {
        Self {
            inherited,
            registry_keys: BTreeSet::new(),
        }
    }

    fn merge(&mut self, snapshot: RegistrySnapshot, overrides: Environment) -> Environment {
        let current_keys = snapshot.keys();
        let mut environment = self.inherited.clone();
        let mut expansion_environment = self.inherited.clone();

        // 进程环境里混有启动时的注册表值。记住历次出现过的键，才能让运行期间
        // 被删除的注册表变量从新 pane 消失，同时保住从父进程继承的私有变量。
        for name in self.registry_keys.iter().chain(&current_keys) {
            environment.remove_normalized(name);
            expansion_environment.remove_normalized(name);
        }

        // HKCU\Environment 里的 REG_EXPAND_SZ 经常引用稍后才从 Volatile
        // Environment 读到的 USERPROFILE。先收集各键最终生效的普通字符串，
        // 让展开不再依赖 hive 的读取顺序；实际输出仍按原顺序叠加，以保留
        // machine/user/volatile 的覆盖及 PATH 追加语义。
        let final_plain_values = snapshot.final_plain_values();
        for entry in final_plain_values.entries.values() {
            expansion_environment.insert(entry.name.clone(), entry.value.clone());
        }
        for hive in snapshot.hives {
            apply_registry_hive(
                &mut environment,
                &mut expansion_environment,
                &final_plain_values,
                &hive,
            );
        }
        for entry in overrides.entries.into_values() {
            environment.insert(entry.name, entry.value);
        }

        self.registry_keys.extend(current_keys);
        environment
    }
}

struct RegistrySnapshot {
    hives: Vec<Vec<RegistryValue>>,
}

impl RegistrySnapshot {
    fn read() -> io::Result<Self> {
        let mut hives = vec![
            read_registry_key(HKEY_LOCAL_MACHINE, SYSTEM_ENVIRONMENT_KEY)?,
            read_registry_key(HKEY_CURRENT_USER, USER_ENVIRONMENT_KEY)?,
            read_registry_key(HKEY_CURRENT_USER, VOLATILE_ENVIRONMENT_KEY)?,
        ];

        let mut session_id = 0;
        let has_session = unsafe { ProcessIdToSessionId(std::process::id(), &mut session_id) } != 0;
        if !has_session {
            return Err(io::Error::last_os_error());
        }
        hives.push(read_registry_key(
            HKEY_CURRENT_USER,
            &format!(r"{VOLATILE_ENVIRONMENT_KEY}\{session_id}"),
        )?);

        Ok(Self { hives })
    }

    fn keys(&self) -> BTreeSet<EnvironmentName> {
        self.hives
            .iter()
            .flatten()
            .map(|value| EnvironmentName::new(&value.name))
            .collect()
    }

    fn final_plain_values(&self) -> Environment {
        let mut final_values = BTreeMap::<EnvironmentName, &RegistryValue>::new();
        for value in self.hives.iter().flatten() {
            if value.kind != RegistryValueKind::Unsupported
                && !value.value.is_empty()
                && !is_path_variable(&value.name)
            {
                final_values.insert(EnvironmentName::new(&value.name), value);
            }
        }

        let mut environment = Environment::default();
        for value in final_values.into_values() {
            if value.kind == RegistryValueKind::String {
                environment.insert(&value.name, &value.value);
            }
        }
        environment
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegistryValueKind {
    String,
    ExpandString,
    Unsupported,
}

#[derive(Clone, Debug)]
struct RegistryValue {
    name: OsString,
    value: OsString,
    kind: RegistryValueKind,
}

fn apply_registry_hive(
    environment: &mut Environment,
    expansion_environment: &mut Environment,
    final_plain_values: &Environment,
    values: &[RegistryValue],
) {
    // REG_SZ 先落表，随后 REG_EXPAND_SZ 才能引用同一 hive 中已经载入的变量。
    for expand_pass in [false, true] {
        for registry_value in values {
            let is_path = is_path_variable(&registry_value.name);
            let is_expand = registry_value.kind == RegistryValueKind::ExpandString
                || (is_path && registry_value.kind == RegistryValueKind::String);
            let supported = registry_value.kind != RegistryValueKind::Unsupported;
            if !supported || is_expand != expand_pass {
                continue;
            }

            let value = if is_expand {
                expand_environment_variables(&registry_value.value, expansion_environment)
            } else {
                registry_value.value.clone()
            };
            if value.is_empty() {
                continue;
            }
            if is_path {
                environment.append_path(registry_value.name.clone(), value.clone());
                expansion_environment.append_path(registry_value.name.clone(), value);
            } else {
                environment.insert(registry_value.name.clone(), value.clone());
                // 最终由普通字符串覆盖的键已提前装入展开上下文。早期 hive
                // 不得把它改回旧值，否则 TEMP 又会看不到后置 USERPROFILE。
                if final_plain_values.get(&registry_value.name).is_none() {
                    expansion_environment.insert(registry_value.name.clone(), value);
                }
            }
        }
    }
}

fn expand_environment_variables(input: &OsStr, environment: &Environment) -> OsString {
    let mut output = Vec::with_capacity(input.len());
    let mut variable = None::<Vec<u16>>;

    for character in input.encode_wide() {
        if character == u16::from(b'%') {
            if let Some(name) = variable.take() {
                if let Some(value) = environment.get(OsString::from_wide(&name)) {
                    output.extend(value.encode_wide());
                } else {
                    output.push(u16::from(b'%'));
                    output.extend(name);
                    output.push(u16::from(b'%'));
                }
            } else {
                variable = Some(Vec::new());
            }
        } else if let Some(name) = variable.as_mut() {
            name.push(character);
        } else {
            output.push(character);
        }
    }

    if let Some(name) = variable {
        output.push(u16::from(b'%'));
        output.extend(name);
    }
    OsString::from_wide(&output)
}

fn is_path_variable(name: &OsStr) -> bool {
    PATH_VARIABLES
        .iter()
        .any(|path_name| name.eq_ignore_ascii_case(OsStr::new(*path_name)))
}

struct RegistryKey(HKEY);

impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn read_registry_key(root: HKEY, path: &str) -> io::Result<Vec<RegistryValue>> {
    let path: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut raw_key = ptr::null_mut();
    let status = unsafe { RegOpenKeyExW(root, path.as_ptr(), 0, KEY_READ, &mut raw_key) };
    if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
        return Ok(Vec::new());
    }
    check_registry_status(status)?;
    let key = RegistryKey(raw_key);

    let mut value_count = 0;
    let mut max_name_length = 0;
    let mut max_data_length = 0;
    let status = unsafe {
        RegQueryInfoKeyW(
            key.0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut value_count,
            &mut max_name_length,
            &mut max_data_length,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    check_registry_status(status)?;

    let mut values = Vec::with_capacity(value_count as usize);
    let mut name_buffer = vec![0_u16; max_name_length.saturating_add(1).max(1) as usize];
    let mut data_buffer = vec![0_u8; max_data_length.max(2) as usize];
    let mut index = 0;

    loop {
        let mut retries = 0;
        let (name_length, data_length, value_type) = loop {
            let mut name_length = name_buffer.len() as u32;
            let mut data_length = data_buffer.len() as u32;
            let mut value_type = 0;
            let status = unsafe {
                RegEnumValueW(
                    key.0,
                    index,
                    name_buffer.as_mut_ptr(),
                    &mut name_length,
                    ptr::null(),
                    &mut value_type,
                    data_buffer.as_mut_ptr(),
                    &mut data_length,
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                return Ok(values);
            }
            if status == ERROR_MORE_DATA && retries < 8 {
                name_buffer.resize(
                    name_buffer
                        .len()
                        .saturating_mul(2)
                        .max(name_length as usize + 1),
                    0,
                );
                data_buffer.resize(
                    data_buffer
                        .len()
                        .saturating_mul(2)
                        .max(data_length as usize)
                        .max(2),
                    0,
                );
                retries += 1;
                continue;
            }
            check_registry_status(status)?;
            break (name_length, data_length, value_type);
        };

        let name = OsString::from_wide(&name_buffer[..name_length as usize]);
        if !name.is_empty() {
            let kind = match value_type {
                REG_SZ => RegistryValueKind::String,
                REG_EXPAND_SZ => RegistryValueKind::ExpandString,
                _ => RegistryValueKind::Unsupported,
            };
            let value = if kind == RegistryValueKind::Unsupported {
                OsString::new()
            } else {
                decode_registry_string(&data_buffer[..data_length as usize])
            };
            values.push(RegistryValue { name, value, kind });
        }
        index += 1;
    }
}

fn decode_registry_string(bytes: &[u8]) -> OsString {
    let mut wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    if let Some(end) = wide.iter().position(|character| *character == 0) {
        wide.truncate(end);
    }
    OsString::from_wide(&wide)
}

fn check_registry_status(status: u32) -> io::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn environment(entries: &[(&str, &str)]) -> Environment {
        let mut environment = Environment::default();
        for (name, value) in entries {
            environment.insert(*name, *value);
        }
        environment
    }

    fn plain(name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> RegistryValue {
        RegistryValue {
            name: name.as_ref().to_os_string(),
            value: value.as_ref().to_os_string(),
            kind: RegistryValueKind::String,
        }
    }

    fn expanded(name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> RegistryValue {
        RegistryValue {
            name: name.as_ref().to_os_string(),
            value: value.as_ref().to_os_string(),
            kind: RegistryValueKind::ExpandString,
        }
    }

    fn snapshot(hives: Vec<Vec<RegistryValue>>) -> RegistrySnapshot {
        RegistrySnapshot { hives }
    }

    #[test]
    fn user_variables_override_machine_variables() {
        let mut state = EnvironmentState::new(Environment::default());
        let merged = state.merge(
            snapshot(vec![
                vec![plain("SDK", "machine")],
                vec![plain("SDK", "user")],
            ]),
            Environment::default(),
        );

        assert_eq!(merged.get("SDK").and_then(OsStr::to_str), Some("user"));
    }

    #[test]
    fn path_variables_append_machine_before_user() {
        let machine = PATH_VARIABLES.map(|name| plain(name, "machine"));
        let user = PATH_VARIABLES.map(|name| plain(name, "user"));
        let mut state = EnvironmentState::new(Environment::default());
        let merged = state.merge(
            snapshot(vec![machine.to_vec(), user.to_vec()]),
            Environment::default(),
        );

        for name in PATH_VARIABLES {
            assert_eq!(
                merged.get(name).and_then(OsStr::to_str),
                Some("machine;user"),
                "{name}"
            );
        }
    }

    #[test]
    fn variable_names_are_case_insensitive() {
        let mut state = EnvironmentState::new(Environment::default());
        let merged = state.merge(
            snapshot(vec![
                vec![plain("MixedCase", "machine")],
                vec![plain("mIXEDcASE", "user")],
            ]),
            Environment::default(),
        );

        assert_eq!(merged.entries.len(), 1);
        assert_eq!(
            merged.get("MIXEDCASE").and_then(OsStr::to_str),
            Some("user")
        );
    }

    #[test]
    fn expandable_values_see_plain_values_from_the_same_hive() {
        let mut state = EnvironmentState::new(Environment::default());
        let merged = state.merge(
            snapshot(vec![vec![
                expanded("SDK", r"%ROOT%\sdk"),
                plain("ROOT", r"C:\tools"),
            ]]),
            Environment::default(),
        );

        assert_eq!(
            merged.get("SDK").and_then(OsStr::to_str),
            Some(r"C:\tools\sdk")
        );
    }

    #[test]
    fn expandable_values_see_plain_values_from_later_hives() {
        // Volatile Environment 排在 HKCU\Environment 之后：TEMP 引用的
        // USERPROFILE 不能按枚举顺序展开，否则 pane 的 TEMP/TMP 会保留字面量，
        // shell 一写临时文件就在启动目录下建出 %USERPROFILE% 文件夹。
        // 用户目录随电脑而不同，直接从当前进程取真实 USERPROFILE，断言值
        // 也由它推导，避免把某台机器的用户名写进测试导致其他电脑失败。
        let profile =
            std::env::var_os("USERPROFILE").expect("USERPROFILE must be set in the test process");
        let mut temp = OsString::from("%USERPROFILE%");
        temp.push(r"\AppData\Local\Temp");
        let mut inherited = Environment::default();
        inherited.insert("USERPROFILE", &profile);
        let mut state = EnvironmentState::new(inherited);
        let merged = state.merge(
            snapshot(vec![
                Vec::new(),
                vec![expanded("TEMP", &temp)],
                vec![plain("USERPROFILE", &profile)],
            ]),
            Environment::default(),
        );

        let mut expected_temp = profile.clone();
        expected_temp.push(r"\AppData\Local\Temp");
        assert_eq!(merged.get("TEMP"), Some(expected_temp.as_os_str()));
        assert_eq!(merged.get("USERPROFILE"), Some(profile.as_os_str()));
    }

    #[test]
    fn later_plain_value_still_overrides_an_earlier_expanded_value() {
        let mut state = EnvironmentState::new(environment(&[("ROOT", r"C:\base")]));
        let merged = state.merge(
            snapshot(vec![
                vec![expanded("PROFILE", r"%ROOT%\machine")],
                vec![plain("PROFILE", r"C:\Users\test")],
            ]),
            Environment::default(),
        );

        assert_eq!(
            merged.get("PROFILE").and_then(OsStr::to_str),
            Some(r"C:\Users\test")
        );
    }

    #[test]
    fn path_stored_as_reg_sz_is_still_expanded() {
        let mut state = EnvironmentState::new(Environment::default());
        let merged = state.merge(
            snapshot(vec![vec![
                plain("Path", r"%ROOT%\bin"),
                plain("ROOT", r"C:\tools"),
            ]]),
            Environment::default(),
        );

        assert_eq!(
            merged.get("PATH").and_then(OsStr::to_str),
            Some(r"C:\tools\bin")
        );
    }

    #[test]
    fn deleted_registry_values_do_not_remove_process_private_values() {
        let mut state = EnvironmentState::new(environment(&[
            ("SESSION_ONLY", "keep"),
            ("RemovedLater", "stale inherited value"),
        ]));
        let first = state.merge(
            snapshot(vec![vec![plain("RemovedLater", "fresh")]]),
            Environment::default(),
        );
        assert_eq!(
            first.get("RemovedLater").and_then(OsStr::to_str),
            Some("fresh")
        );

        let second = state.merge(snapshot(vec![Vec::new()]), Environment::default());
        assert_eq!(second.get("RemovedLater"), None);
        assert_eq!(
            second.get("SESSION_ONLY").and_then(OsStr::to_str),
            Some("keep")
        );
    }

    #[test]
    fn pane_overrides_are_applied_after_the_registry_snapshot() {
        let mut state = EnvironmentState::new(Environment::default());
        let merged = state.merge(
            snapshot(vec![vec![plain("PaneValue", "registry")]]),
            environment(&[("pANEvALUE", "pane")]),
        );

        assert_eq!(
            merged.get("PANEvalue").and_then(OsStr::to_str),
            Some("pane")
        );
        assert_eq!(merged.entries.len(), 1);
    }

    struct ProcessEnvRestore {
        name: std::ffi::OsString,
        previous: Option<std::ffi::OsString>,
    }

    impl ProcessEnvRestore {
        fn set(name: std::ffi::OsString, value: &OsStr) -> Self {
            let previous = std::env::var_os(&name);
            unsafe { std::env::set_var(&name, value) };
            Self { name, previous }
        }
    }

    impl Drop for ProcessEnvRestore {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                unsafe { std::env::set_var(&self.name, previous) };
            } else {
                unsafe { std::env::remove_var(&self.name) };
            }
        }
    }

    #[test]
    fn process_snapshot_preserves_non_unicode_values() {
        use std::os::windows::ffi::OsStringExt;

        let _lock = crate::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let name = std::ffi::OsString::from("SLTERM_NON_UNICODE_REFRESH_TEST");
        let value = std::ffi::OsString::from_wide(&[u16::from(b'X'), 0xD800, u16::from(b'Y')]);
        let _restore = ProcessEnvRestore::set(name, &value);

        let environment = Environment::from_process();
        let actual = environment
            .get("SLTERM_NON_UNICODE_REFRESH_TEST")
            .expect("进程私有变量必须进入快照");
        assert_eq!(
            OsStr::new(actual).encode_wide().collect::<Vec<_>>(),
            value.encode_wide().collect::<Vec<_>>()
        );
    }

    #[test]
    fn process_snapshot_preserves_non_unicode_names() {
        use std::os::windows::ffi::OsStringExt;

        let _lock = crate::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut name_wide: Vec<u16> = "SLTERM_NON_UNICODE_NAME_".encode_utf16().collect();
        name_wide.push(0xD800);
        let name = std::ffi::OsString::from_wide(&name_wide);
        let value = std::ffi::OsString::from("preserved");
        let _restore = ProcessEnvRestore::set(name, &value);

        let environment = Environment::from_process();
        let entry = environment
            .entries
            .values()
            .find(|entry| OsStr::new(&entry.name).encode_wide().collect::<Vec<_>>() == name_wide)
            .expect("非 Unicode 变量名不得被有损改写");
        assert_eq!(
            OsStr::new(&entry.value).encode_wide().collect::<Vec<_>>(),
            value.encode_wide().collect::<Vec<_>>()
        );
    }

    #[test]
    fn registry_string_decoding_preserves_unpaired_surrogates() {
        let wide = [u16::from(b'X'), 0xD800, u16::from(b'Y'), 0];
        let bytes: Vec<u8> = wide.iter().flat_map(|unit| unit.to_le_bytes()).collect();

        let decoded = decode_registry_string(&bytes);

        assert_eq!(
            OsStr::new(&decoded).encode_wide().collect::<Vec<_>>(),
            wide[..3]
        );
    }

    #[test]
    fn expansion_preserves_non_unicode_values() {
        use std::os::windows::ffi::OsStringExt;

        let value = std::ffi::OsString::from_wide(&[u16::from(b'X'), 0xD800, u16::from(b'Y')]);
        let mut environment = Environment::default();
        environment.insert("ROOT", value.clone());

        let expanded = expand_environment_variables(OsStr::new("%ROOT%\\bin"), &environment);
        let expected: Vec<u16> = value.encode_wide().chain("\\bin".encode_utf16()).collect();
        assert_eq!(expanded.encode_wide().collect::<Vec<_>>(), expected);
    }
}
