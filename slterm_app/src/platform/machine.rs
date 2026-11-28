//! 机器标识：窗口几何按 machine_key 隔离，跨机共享数据库不互相覆盖。
//!
//! 标识只用于本地持久化行键，不进入日志、诊断包或网络请求。

/// 诊断快照使用的 OS 身份；平台差异只在本层。
pub(crate) fn os_identity() -> String {
    #[cfg(windows)]
    {
        format!("windows-{}", super::material::windows_build_number())
    }
    #[cfg(not(windows))]
    {
        "non-windows".to_owned()
    }
}

/// 规范化机器名：空白与缺失落到稳定占位，避免空主键。
fn machine_key_from(name: Option<String>) -> String {
    name.map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown-machine".to_owned())
}

/// 当前机器标识；测试构建固定值，不读取真实主机名。
pub(crate) fn machine_key() -> String {
    #[cfg(test)]
    {
        "test-machine".to_owned()
    }
    #[cfg(not(test))]
    {
        machine_key_from(host_name())
    }
}

/// 本机 DNS 主机名；失败返回 None 由调用方落到占位。
#[cfg(all(windows, not(test)))]
fn host_name() -> Option<String> {
    use windows_sys::Win32::System::SystemInformation::{
        ComputerNameDnsHostname, GetComputerNameExW,
    };
    let mut size = 0u32;
    // SAFETY: 第一次调用只为取得所需长度，不写缓冲区。
    unsafe {
        GetComputerNameExW(ComputerNameDnsHostname, std::ptr::null_mut(), &mut size);
    }
    if size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; size as usize];
    // SAFETY: 缓冲区按系统给出的长度分配，size 同时作为输入容量。
    let ok =
        unsafe { GetComputerNameExW(ComputerNameDnsHostname, buffer.as_mut_ptr(), &mut size) != 0 };
    if !ok {
        return None;
    }
    String::from_utf16(&buffer[..size as usize]).ok()
}

#[cfg(all(not(windows), not(test)))]
fn host_name() -> Option<String> {
    None
}

#[cfg(test)]
mod machine_tests {
    use super::*;

    /// 空名与缺失都落到稳定占位，不产生空主键。
    #[test]
    fn machine_key_never_returns_an_empty_identity() {
        assert_eq!(machine_key_from(None), "unknown-machine");
        assert_eq!(machine_key_from(Some(String::new())), "unknown-machine");
        assert_eq!(machine_key_from(Some("   ".to_owned())), "unknown-machine");
        assert_eq!(
            machine_key_from(Some(" desktop-01 ".to_owned())),
            "desktop-01"
        );
    }

    /// 当前进程机器标识在测试构建固定，避免依赖真实主机名。
    #[test]
    fn machine_key_is_stable_within_the_test_build() {
        assert_eq!(machine_key(), "test-machine");
    }
}
