//! LocalControlServer 非敏感发现记录的安全发布、读取与撤销。

use std::fs::File;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};

use slterm_runtime_protocol::DiscoveryRecord;

use crate::atomic_file::{FileIdentity, remove_if_matches, write_current_user};
use crate::platform::dirs;

// 五个定长/短身份字段的发现记录预算；包含 UUID、SocketAddr 与 JSON 转义余量。
const DISCOVERY_FILE_BUDGET: usize = 4 * 1024;
const ENDPOINT_FILE_NAME: &str = "local-control-endpoint.json";

fn endpoint_path() -> PathBuf {
    dirs::local_dir().join(ENDPOINT_FILE_NAME)
}

/// 首实例发布本次运行记录；认证材料不经过此通道。
#[allow(dead_code)] // 消费点 = LocalControlServer 启动接线。
pub(crate) fn publish(record: DiscoveryRecord) -> io::Result<DiscoveryGuard> {
    publish_at(&endpoint_path(), record)
}

fn publish_at(path: &Path, record: DiscoveryRecord) -> io::Result<DiscoveryGuard> {
    record
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let contents = serde_json::to_vec(&record)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if contents.len() > DISCOVERY_FILE_BUDGET {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "控制面发现记录超过 4 KiB 合同预算",
        ));
    }
    let identity = write_current_user(path, &contents)?;
    Ok(DiscoveryGuard {
        path: path.to_owned(),
        contents,
        identity,
        active: true,
    })
}

/// 客户端只读加载；缺失、损坏、超限与非法端点均明确失败。
#[allow(dead_code)] // 消费点 = Runtime discover 与客户端接线。
pub(crate) fn read_discovery() -> io::Result<DiscoveryRecord> {
    read_discovery_at(&endpoint_path())
}

fn read_discovery_at(path: &Path) -> io::Result<DiscoveryRecord> {
    let file = File::open(path)?;
    let mut contents = Vec::new();
    file.take(DISCOVERY_FILE_BUDGET as u64 + 1)
        .read_to_end(&mut contents)?;
    if contents.len() > DISCOVERY_FILE_BUDGET {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "控制面发现记录超过 4 KiB 合同预算",
        ));
    }
    let record: DiscoveryRecord = serde_json::from_slice(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    record
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(record)
}

/// 只拥有本次原子发布的文件身份；撤销不会沿路径删除替代记录。
pub(crate) struct DiscoveryGuard {
    path: PathBuf,
    contents: Vec<u8>,
    identity: FileIdentity,
    active: bool,
}

impl DiscoveryGuard {
    /// 显式撤销保留所有错误；调用方可在 shutdown 链中上报。
    #[allow(dead_code)] // 消费点 = LocalControlServer 正常关闭接线。
    pub(crate) fn revoke(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        remove_if_matches(&self.path, self.identity, &self.contents)?;
        self.active = false;
        Ok(())
    }
}

impl Drop for DiscoveryGuard {
    fn drop(&mut self) {
        // Drop 无法返回错误；正常关闭必须先调用显式 revoke。
        let _ = self.revoke();
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::path::Path;
    use std::ptr;

    use slterm_runtime_protocol::DiscoveryRecord;
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, GetLastError, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
        GetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, GetTokenInformation, PSECURITY_DESCRIPTOR, TOKEN_QUERY,
        TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    use super::{DiscoveryGuard, publish_at, read_discovery_at};

    fn record(run_id: &str) -> DiscoveryRecord {
        DiscoveryRecord {
            protocol_version: 2,
            endpoint: "127.0.0.1:4242".parse().unwrap(),
            run_id: run_id.to_owned(),
            process_id: 123,
            process_created_at_ms: 456,
        }
    }

    fn current_user_sid() -> io::Result<String> {
        let mut token = ptr::null_mut();
        // SAFETY: 伪句柄只用于本调用；成功取得的 token 在闭包返回后关闭。
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let result = (|| {
            let mut byte_len = 0_u32;
            // SAFETY: 首次以空缓冲查询所需长度，输出长度指针有效。
            let queried =
                unsafe { GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut byte_len) };
            if queried != 0 || byte_len == 0 {
                // SAFETY: 上一 Win32 调用刚失败，线程错误码尚未被其他 API 覆盖。
                return Err(io::Error::from_raw_os_error(
                    unsafe { GetLastError() } as i32
                ));
            }
            let mut buffer = vec![0_usize; byte_len.div_ceil(size_of::<usize>() as u32) as usize];
            // SAFETY: usize 缓冲满足对齐且容量不小于 byte_len；SID 生命周期受缓冲保护。
            if unsafe {
                GetTokenInformation(
                    token,
                    TokenUser,
                    buffer.as_mut_ptr().cast(),
                    byte_len,
                    &mut byte_len,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: 成功的 TokenUser 查询保证缓冲以 TOKEN_USER 开头。
            let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
            let mut text = ptr::null_mut();
            // SAFETY: SID 来自有效令牌用户；text 由系统分配并在复制后释放。
            if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: ConvertSidToStringSidW 成功返回 NUL 结尾 UTF-16 字符串。
            let value = unsafe {
                let mut len = 0_usize;
                while *text.add(len) != 0 {
                    len += 1;
                }
                String::from_utf16(std::slice::from_raw_parts(text, len))
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
            };
            // SAFETY: text 由 ConvertSidToStringSidW 分配，且不再使用。
            unsafe { LocalFree(text.cast()) };
            value
        })();
        // SAFETY: token 由 OpenProcessToken 成功返回，闭包已不再使用。
        unsafe { windows_sys::Win32::Foundation::CloseHandle(token) };
        result
    }

    fn dacl_sddl(path: &Path) -> io::Result<String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
        // SAFETY: 路径与输出指针有效；安全描述符由系统分配并在本函数释放。
        let status = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let result = (|| {
            let mut text = ptr::null_mut();
            let mut length = 0_u32;
            // SAFETY: descriptor 来自成功查询；text 由系统分配并在复制后释放。
            if unsafe {
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor,
                    SDDL_REVISION_1,
                    DACL_SECURITY_INFORMATION,
                    &mut text,
                    &mut length,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: 转换成功时 text 指向 length 个 UTF-16 码单元。
            let value = unsafe {
                String::from_utf16_lossy(std::slice::from_raw_parts(text, length as usize))
            };
            let value = value.trim_end_matches('\0').to_owned();
            // SAFETY: text 由转换 API 分配，且不再使用。
            unsafe { LocalFree(text.cast()) };
            Ok(value)
        })();
        // SAFETY: descriptor 由 GetNamedSecurityInfoW 分配，闭包已不再使用。
        unsafe { LocalFree(descriptor) };
        result
    }

    fn assert_current_user_only_acl(path: &Path) {
        let sid = current_user_sid().unwrap();
        let dacl = dacl_sddl(path).unwrap();
        let (control, aces) = dacl.split_once('(').expect("SDDL 必须包含 ACE");
        assert!(
            matches!(control, "D:P" | "D:PAI"),
            "DACL 必须禁止继承: {dacl}"
        );
        assert_eq!(
            format!("({aces}"),
            format!("(A;;FA;;;{sid})"),
            "唯一 ACE 必须是当前用户的显式完全控制"
        );
    }

    #[test]
    fn publish_read_and_revoke_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let expected = record("run-test");
        let mut guard = publish_at(&path, expected.clone()).unwrap();

        assert_eq!(read_discovery_at(&path).unwrap(), expected);
        guard.revoke().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn drop_best_effort_revokes_owned_record() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let guard: DiscoveryGuard = publish_at(&path, record("run-test")).unwrap();
        assert!(path.exists());

        drop(guard);
        assert!(!path.exists());
    }

    #[test]
    fn corrupted_unknown_non_loopback_and_oversized_records_fail() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let mut with_token = serde_json::to_value(record("run-test")).unwrap();
        with_token["token"] = serde_json::Value::String("sk-test".to_owned());
        let non_loopback = serde_json::to_vec(&DiscoveryRecord {
            endpoint: "192.168.1.2:4242".parse().unwrap(),
            ..record("run-test")
        })
        .unwrap();
        for contents in [
            vec![0xff, 0xfe],
            b"not json".to_vec(),
            serde_json::to_vec(&with_token).unwrap(),
            non_loopback,
            vec![b' '; 4097],
        ] {
            std::fs::write(&path, contents).unwrap();
            let error = read_discovery_at(&path).expect_err("非法发现记录必须明确失败");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }

    #[test]
    fn old_guard_cannot_revoke_replacement_even_with_identical_contents() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let expected = record("run-test");
        let mut old = publish_at(&path, expected.clone()).unwrap();
        let mut replacement = publish_at(&path, expected.clone()).unwrap();

        assert!(old.revoke().is_err(), "旧 guard 必须观察到所有权冲突");
        drop(old);
        assert_eq!(read_discovery_at(&path).unwrap(), expected);
        replacement.revoke().unwrap();
    }

    #[test]
    fn guard_cannot_revoke_record_modified_in_place() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let mut guard = publish_at(&path, record("run-old")).unwrap();
        let replacement = record("run-new");
        std::fs::write(&path, serde_json::to_vec(&replacement).unwrap()).unwrap();

        assert!(guard.revoke().is_err(), "内容变化必须撤销失败");
        drop(guard);
        assert_eq!(read_discovery_at(&path).unwrap(), replacement);
    }

    #[test]
    fn revocation_excludes_readers_until_directory_entry_is_removed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let expected = record("run-test");
        let mut guard = publish_at(&path, expected.clone()).unwrap();
        let reader = OpenOptions::new()
            .read(true)
            .share_mode(
                windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                    | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE
                    | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
            )
            .open(&path)
            .unwrap();

        assert!(guard.revoke().is_err(), "读者存活时不得进入 delete-pending");
        assert!(std::fs::symlink_metadata(&path).unwrap().is_file());
        drop(reader);
        guard.revoke().unwrap();
        let error = std::fs::symlink_metadata(&path).unwrap_err();
        assert_eq!(
            error.kind(),
            io::ErrorKind::NotFound,
            "Ok 返回时目录项必须已删除"
        );
    }

    #[test]
    fn revocation_excludes_writers_during_ownership_check() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let expected = record("run-test");
        let mut guard = publish_at(&path, expected.clone()).unwrap();
        let writer = OpenOptions::new()
            .write(true)
            .share_mode(
                windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                    | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE
                    | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
            )
            .open(&path)
            .unwrap();

        assert!(guard.revoke().is_err(), "写者存活时不得判定并删除记录");
        drop(writer);
        assert_eq!(read_discovery_at(&path).unwrap(), expected);
    }

    #[test]
    fn explicit_revoke_reports_os_errors() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let mut guard = publish_at(&path, record("run-test")).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();

        assert!(guard.revoke().is_err(), "显式撤销不得吞掉 OS 错误");
        assert!(path.is_dir());
    }

    #[test]
    fn failed_replacement_preserves_the_previous_record() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let previous = record("run-old");
        let _guard = publish_at(&path, previous.clone()).unwrap();
        let blocker = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();

        assert!(publish_at(&path, record("run-new")).is_err());
        drop(blocker);
        assert_eq!(read_discovery_at(&path).unwrap(), previous);
    }

    #[test]
    fn published_record_has_only_current_user_explicit_ace() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        let _guard = publish_at(&path, record("run-test")).unwrap();

        assert_current_user_only_acl(&path);
    }

    #[test]
    fn replacement_converges_an_existing_wide_acl() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("endpoint.json");
        std::fs::write(&path, b"old").unwrap();
        let granted = std::process::Command::new("icacls")
            .arg(&path)
            .args(["/grant", "*S-1-1-0:(R)"])
            .output()
            .unwrap();
        assert!(granted.status.success(), "测试必须真实写入宽 ACL");
        let wide_dacl = dacl_sddl(&path).unwrap();
        assert!(
            wide_dacl.contains("WD") || wide_dacl.contains("S-1-1-0"),
            "宽 ACL 必须真实出现在 DACL 中: {wide_dacl}"
        );

        let _guard = publish_at(&path, record("run-test")).unwrap();
        assert_current_user_only_acl(&path);
    }
}
