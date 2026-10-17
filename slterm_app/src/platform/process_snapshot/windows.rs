//! 不打开受保护进程即可同时读出进程 id 与创建时间。
//! 父 PID 复用由共享 process-tree 规则消解(`crate::process_tree`)。

use super::ProcessRow;
use std::{mem, ptr};

use windows_sys::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SystemProcessInformation,
};
use windows_sys::Win32::Foundation::{
    HANDLE, STATUS_BUFFER_TOO_SMALL, STATUS_INFO_LENGTH_MISMATCH, UNICODE_STRING,
};

// Vista 起 SystemProcessInformation 记录到 InheritedFromUniqueProcessId 为止
// 的前缀。Windows SDK 把 CreateTime 与父进程字段列为保留位;本信息类的原生
// 布局把它们放在这里。适配器只认前缀,每条变长记录都做边界校验,并在原生
// 回归测试里拿 CreateTime 与独立 Win32 API 对拍。
#[repr(C)]
#[derive(Clone, Copy)]
struct ProcessHeader {
    next_offset: u32,
    threads: u32,
    private_working_set: i64,
    hard_faults: u32,
    thread_high_watermark: u32,
    cycle_time: u64,
    created: i64,
    user_time: i64,
    kernel_time: i64,
    name: UNICODE_STRING,
    base_priority: i32,
    pid: HANDLE,
    parent: HANDLE,
}

pub(super) fn snapshot() -> Result<Vec<ProcessRow>, String> {
    const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;
    let mut storage = vec![0usize; 64 * 1024 / mem::size_of::<usize>()];
    loop {
        let capacity = mem::size_of_val(storage.as_slice());
        let mut needed = 0;
        let status = unsafe {
            NtQuerySystemInformation(
                SystemProcessInformation,
                storage.as_mut_ptr().cast(),
                capacity as u32,
                &mut needed,
            )
        };
        if matches!(
            status,
            STATUS_INFO_LENGTH_MISMATCH | STATUS_BUFFER_TOO_SMALL
        ) {
            let next = (needed as usize)
                .saturating_add(64 * 1024)
                .max(capacity * 2);
            if next > MAX_SNAPSHOT_BYTES {
                return Err("process snapshot exceeds the 16 MiB inspection limit".to_owned());
            }
            storage.resize(next.div_ceil(mem::size_of::<usize>()), 0);
            continue;
        }
        if status < 0 {
            return Err(format!(
                "NtQuerySystemInformation failed: NTSTATUS {status:#x}"
            ));
        }
        if needed == 0 || needed as usize > capacity {
            return Err("invalid process snapshot length".to_owned());
        }
        // 原生字符串指针指向本次分配,解析器把每个名字拷进自持的 ProcessRow
        // 之前这块内存一直活着。
        let bytes =
            unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), needed as usize) };
        return parse_processes(bytes);
    }
}

fn parse_processes(bytes: &[u8]) -> Result<Vec<ProcessRow>, String> {
    let mut rows = Vec::new();
    let mut offset = 0usize;
    loop {
        let remaining = bytes.get(offset..).ok_or("invalid process record offset")?;
        if remaining.len() < mem::size_of::<ProcessHeader>() {
            return Err("truncated process record".to_owned());
        }
        let header = unsafe { ptr::read_unaligned(remaining.as_ptr().cast::<ProcessHeader>()) };
        let pid = u32::try_from(header.pid as usize).map_err(|_| "invalid process id")?;
        let parent = u32::try_from(header.parent as usize).map_err(|_| "invalid parent id")?;
        let created = u64::try_from(header.created).map_err(|_| "invalid process creation time")?;
        let executable = read_name(bytes, header.name)?;
        rows.push(ProcessRow {
            pid,
            parent,
            executable,
            created,
        });
        let step = header.next_offset as usize;
        if step == 0 {
            return Ok(rows);
        }
        if step < mem::size_of::<ProcessHeader>() || step > remaining.len() {
            return Err("invalid next process record offset".to_owned());
        }
        offset += step;
    }
}

fn read_name(bytes: &[u8], name: UNICODE_STRING) -> Result<String, String> {
    let length = name.Length as usize;
    if length == 0 {
        return Ok(String::new());
    }
    let start = (name.Buffer as usize)
        .checked_sub(bytes.as_ptr() as usize)
        .ok_or("process name lies outside the snapshot")?;
    if !length.is_multiple_of(2) || length > name.MaximumLength as usize {
        return Err("invalid process name length".to_owned());
    }
    let end = start
        .checked_add(length)
        .ok_or("process name range overflow")?;
    let encoded = bytes.get(start..end).ok_or("truncated process name")?;
    // 逐字节对读,避免对内核对齐的指针强加对齐要求。
    Ok(char::decode_utf16(
        encoded
            .chunks_exact(2)
            .map(|pair| u16::from_ne_bytes([pair[0], pair[1]])),
    )
    .map(|value| value.unwrap_or(char::REPLACEMENT_CHARACTER))
    .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_native_records_are_rejected() {
        assert!(parse_processes(&[0u8; 8]).is_err());
        let mut data = vec![0u8; mem::size_of::<ProcessHeader>()];
        let mut header: ProcessHeader = unsafe { mem::zeroed() };
        header.next_offset = 1;
        unsafe { ptr::write_unaligned(data.as_mut_ptr().cast::<ProcessHeader>(), header) };
        assert!(parse_processes(&data).is_err());
        header.next_offset = 0;
        header.name.Length = 2;
        header.name.MaximumLength = 2;
        header.name.Buffer = ptr::null_mut();
        unsafe { ptr::write_unaligned(data.as_mut_ptr().cast::<ProcessHeader>(), header) };
        assert!(parse_processes(&data).is_err());
    }

    #[test]
    fn native_creation_time_matches_the_win32_process_clock() {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

        let rows = snapshot().unwrap();
        let current = rows
            .iter()
            .find(|row| row.pid == std::process::id())
            .unwrap();
        assert!(!current.executable.is_empty());
        let mut times = [unsafe { mem::zeroed() }; 4];
        let [created, exited, kernel, user] = &mut times;
        assert_ne!(
            unsafe { GetProcessTimes(GetCurrentProcess(), created, exited, kernel, user) },
            0
        );
        let expected = u64::from(created.dwHighDateTime) << 32 | u64::from(created.dwLowDateTime);
        assert_eq!(current.created, expected);
    }

    #[test]
    fn native_snapshot_preserves_a_live_childs_parent() {
        use std::process::{Child, Command, Stdio};

        struct OwnedChild(Child);
        impl Drop for OwnedChild {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        // shell 等我们的管道输入,快照不会撞上它的自然退出;断言失败时 guard
        // 也负责回收子进程。
        let child = OwnedChild(
            Command::new("cmd.exe")
                .args(["/d", "/q", "/c", "set /p _SLTERM_PARENT_TEST="])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let rows = snapshot().unwrap();
        let entry = rows.iter().find(|row| row.pid == child.0.id()).unwrap();
        assert_eq!(entry.parent, std::process::id());
        assert!(entry.executable.eq_ignore_ascii_case("cmd.exe"));
    }
}
