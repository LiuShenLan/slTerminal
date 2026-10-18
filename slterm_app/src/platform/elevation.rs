//! 进程提权态探针(会话持久化 `isolated` 语义的判据)。
//!
//! 管理员令牌与普通令牌共用同一数据目录时,管理员窗口的持久化不能覆盖
//! 普通工作区,更不能把提权 shell 命令写进下次普通启动的恢复快照。单实例
//! 下这与 `atomic_file` 的跨进程锁是两道正交防线:锁保写入互斥,本探针保
//! 「提权进程根本不写共享会话」。
//!
//! 只回答「当前令牌是否提权」;不解释 UAC 类型,也不做降权/提权动作。

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// 当前进程是否运行在提权令牌下(管理员)。探针失败按未提权处理——宁可
/// 写入一次共享快照,也不因一次 API 失败让正常进程彻底不保存会话。
pub(crate) fn requires_isolation() -> bool {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: 伪句柄只在本调用内使用;token 成功时由本函数关闭。
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
    if opened == 0 {
        return false;
    }
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut returned = 0u32;
    // SAFETY: 输出缓冲与长度均指向本栈帧的有效内存。
    let queried = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&raw mut elevation).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    };
    // SAFETY: token 由 OpenProcessToken 成功返回,所有权在本函数。
    unsafe { CloseHandle(token) };
    queried != 0 && elevation.TokenIsElevated != 0
}

#[cfg(test)]
mod elevation_tests {
    use super::*;

    /// 探针必须幂等且不 panic;两种结果都合法(CI/开发机身份不同)。
    #[test]
    fn elevation_probe_is_deterministic_and_callable() {
        let first = requires_isolation();
        assert_eq!(first, requires_isolation());
    }
}
