// fs_read_file_range_tests.rs — fs_read_file_range 区间读取测试（CP-022 大文件只读分片浏览）
//
// 直接测同步核心 read_file_range（无需构造 tauri::State——与 read_file_chunked_tests
// 同模式）；命令内核分支（sandbox）测 fs_read_file_range_impl。
// 覆盖: 区间精确读取 / 空区间 / 越界 clamp / 零长度 / 多字节字符头回溯+尾裁剪不截断 /
// 逐块拼接还原（跨块字符归属单块）/ 不存在路径 AppError / sandbox 分支。

use std::path::PathBuf;

use super::fs_read_file_range_impl;
use super::read_file_range;

fn run<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

/// 写临时文件并返回（目录, 路径）
fn temp_file(name: &str, bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    (dir, path)
}

// ── 区间精确读取 ──────────────────────────────────────────────

/// 中段区间读取: 返回 [offset, offset+length) 原文（纯 ASCII 无边界调整）
#[test]
fn range_reads_exact_slice() {
    let (_dir, path) = temp_file("a.txt", b"hello world");
    let text = read_file_range(&path.to_string_lossy(), 6, 5).unwrap();
    assert_eq!(text, "world", "中段区间应返回精确字节子串");
}

/// offset=0 + length=整文件 → 全量还原
#[test]
fn range_read_whole_file_from_zero() {
    let content = "第一行\nsecond line\nthird\n";
    let (_dir, path) = temp_file("b.txt", content.as_bytes());
    let text = read_file_range(&path.to_string_lossy(), 0, content.len() as u64).unwrap();
    assert_eq!(text, content, "offset=0 全量读取应还原原文");
}

/// length 超出文件余量 → clamp 到 EOF（只返回实际存在字节）
#[test]
fn range_read_clamped_to_eof() {
    let (_dir, path) = temp_file("c.txt", b"0123456789");
    let text = read_file_range(&path.to_string_lossy(), 3, 1000).unwrap();
    assert_eq!(text, "3456789", "越界区间应钳制到 EOF 而非报错");
}

// ── 空区间 ────────────────────────────────────────────────────

/// offset 恰在 EOF → 空串（不触碰文件）
#[test]
fn range_read_offset_at_eof_returns_empty() {
    let (_dir, path) = temp_file("d.txt", b"abc");
    let text = read_file_range(&path.to_string_lossy(), 3, 100).unwrap();
    assert_eq!(text, "", "offset 在 EOF 处应返回空串");
}

/// offset 越过 EOF（含 u64 极值，防 seek/加法溢出）→ 空串
#[test]
fn range_read_offset_beyond_eof_returns_empty() {
    let (_dir, path) = temp_file("e.txt", b"abc");
    let text = read_file_range(&path.to_string_lossy(), u64::MAX, 100).unwrap();
    assert_eq!(text, "", "offset 越过 EOF 应返回空串（不得溢出/panic）");
    let text2 = read_file_range(&path.to_string_lossy(), 100, u64::MAX - 50).unwrap();
    assert_eq!(text2, "", "length 极值时按 EOF 余量钳制读取");
}

/// 零长度区间 → 空串（前置短路）
#[test]
fn range_read_zero_length_returns_empty() {
    let (_dir, path) = temp_file("f.txt", b"content");
    let text = read_file_range(&path.to_string_lossy(), 1, 0).unwrap();
    assert_eq!(text, "", "length=0 应直接返回空串");
}

/// 空文件 → 空串（任意区间）
#[test]
fn range_read_empty_file_returns_empty() {
    let (_dir, path) = temp_file("g.txt", b"");
    let text = read_file_range(&path.to_string_lossy(), 0, 100).unwrap();
    assert_eq!(text, "", "空文件任意区间应返回空串");
}

// ── 多字节字符边界 ────────────────────────────────────────────

/// 尾裁剪: 读取区间末端落在多字节字符中段 → 裁掉半截字符（返回纯合法 UTF-8）
#[test]
fn range_read_tail_not_split_multibyte() {
    // 「界」= E7 95 8C 位于字节 2..5; 读 [0,4) 末端切在字符中部
    let bytes = [b'x', b'y', 0xE7, 0x95, 0x8C, b'z'];
    let (_dir, path) = temp_file("h.txt", &bytes);
    let text = read_file_range(&path.to_string_lossy(), 0, 4).unwrap();
    assert_eq!(text, "xy", "尾部半截字符应被裁掉（不得产生非法 UTF-8）");
    // 全量读取对照: 完整字符不丢
    let full = read_file_range(&path.to_string_lossy(), 0, bytes.len() as u64).unwrap();
    assert_eq!(full, "xy界z", "未触及边界的读取应完整还原");
}

/// 头回溯（3 字节字符）: 请求起点落在字符中段 → 回溯包含整字符
#[test]
fn range_read_head_back_includes_straddling_char() {
    // 字节: a(0) E7(1) 95(2) 8C(3) b(4) = "a界b"
    let bytes = [b'a', 0xE7, 0x95, 0x8C, b'b'];
    let (_dir, path) = temp_file("i.txt", &bytes);
    // 起点 = 字符末字节（3）→ 回溯到 E7 整字符返回
    let text = read_file_range(&path.to_string_lossy(), 3, 1).unwrap();
    assert_eq!(text, "界", "起点在字符中段应回溯含整字符");
    // 起点 = 字符中字节（2）, length 2 → 回溯后覆盖 [1,4) = 整字符（b 超出请求区间）
    let text2 = read_file_range(&path.to_string_lossy(), 2, 2).unwrap();
    assert_eq!(text2, "界", "中段起点回溯后整字符在请求窗口内");
    // 起点 = 字符中字节（2）, length=1（只含字符中部）→ 无可解码完整字符 → 空串
    let text3 = read_file_range(&path.to_string_lossy(), 2, 1).unwrap();
    assert_eq!(text3, "", "单字节区间落在字符中段时无可解码内容");
}

/// 头回溯（4 字节 emoji）: 起点在其第 2/4 字节 → 均回溯首字节完整返回
#[test]
fn range_read_head_back_4byte_char() {
    // 「😀」= F0 9F 98 80 位于字节 6..10（"start " 6 字节）
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"start ");
    bytes.extend_from_slice("😀".as_bytes());
    bytes.push(b'!');
    let (_dir, path) = temp_file("j.txt", &bytes);

    // 起点 = emoji 末字节 → 回溯整 emoji
    let text = read_file_range(&path.to_string_lossy(), 9, 1).unwrap();
    assert_eq!(text, "😀", "4 字节字符末字节起点应回溯整字符");
    // 起点 = emoji 第 3 字节, length 2 → 回溯后整字符在窗口内
    let text2 = read_file_range(&path.to_string_lossy(), 8, 2).unwrap();
    assert_eq!(text2, "😀", "4 字节字符中段起点应回溯整字符");
    // 起点 = emoji 首字节 + 完整 4 字节 → 正常整字符
    let text3 = read_file_range(&path.to_string_lossy(), 6, 4).unwrap();
    assert_eq!(text3, "😀", "完整字符区间应原样返回");
}

/// 逐块拼接还原: 以块首递增顺序请求, 拼接 = 原文（分片浏览核心契约）
///
/// 字符「界」从块边界前 1 字节开始跨两请求——头回溯/尾裁剪保证其完整归属单块,
/// 两响应拼接零间隙零重叠。
#[test]
fn range_read_sequential_blocks_stitch_exact() {
    const BLOCK: u64 = 256 * 1024;
    let mut content = vec![b'a'; BLOCK as usize - 1];
    content.extend_from_slice("界".as_bytes());
    content.extend_from_slice(&"后".repeat(1000).as_bytes());
    let (_dir, path) = temp_file("k.txt", &content);

    let b0 = read_file_range(&path.to_string_lossy(), 0, BLOCK).unwrap();
    let b1 = read_file_range(&path.to_string_lossy(), BLOCK, BLOCK).unwrap();
    let stitched: String = b0.clone() + &b1;
    assert_eq!(
        stitched.as_bytes(),
        &content[..],
        "逐块拼接应与原文一致（跨块字符不切散不丢失）"
    );
    assert!(
        !b0.ends_with('\u{FFFD}') && !b1.starts_with('\u{FFFD}'),
        "块边界不得产生替换字符"
    );
}

// ── 不存在路径 ────────────────────────────────────────────────

/// 不存在的文件 → AppError（消息含路径上下文, BE-13）
#[test]
fn range_read_nonexistent_path_errors() {
    let dir = tempfile::tempdir().unwrap();
    let ghost = dir.path().join("ghost.txt");
    let err = read_file_range(&ghost.to_string_lossy(), 0, 10).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains(&ghost.to_string_lossy().to_string()),
        "错误消息应含完整路径, 实际: {msg}"
    );
    let _ = ghost;
}

// ── 命令内核分支: sandbox（HFN-08 直调 impl）──────────────────

/// 根外路径 → 沙箱拒绝
#[test]
fn range_read_outside_root_rejected() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let file = outside.path().join("secret.txt");
    std::fs::write(&file, "secret").unwrap();

    let result = run(fs_read_file_range_impl(
        file.to_string_lossy().to_string(),
        0,
        10,
        Some(root.path().to_path_buf()),
    ));
    assert!(result.is_err(), "根外路径应被沙箱拒绝");
}

/// project_root 未设置（EnforceRootCheckGuard 置位强制校验）→ 拒绝
#[test]
fn range_read_without_root_rejected() {
    let _guard = crate::state::EnforceRootCheckGuard::enforce();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, "x").unwrap();

    let result = run(fs_read_file_range_impl(
        file.to_string_lossy().to_string(),
        0,
        10,
        None,
    ));
    assert!(result.is_err(), "project_root 未设置应拒绝");
}

/// 根内路径 → 成功（impl 层经沙箱后正常读）
#[test]
fn range_read_inside_root_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("ok.txt");
    std::fs::write(&file, "hello world").unwrap();

    let text = run(fs_read_file_range_impl(
        file.to_string_lossy().to_string(),
        6,
        5,
        Some(dir.path().to_path_buf()),
    ))
    .unwrap();
    assert_eq!(text, "world");
}

/// 不存在的根内路径 → AppError 传播（impl 层）
#[test]
fn range_read_impl_nonexistent_errors() {
    let dir = tempfile::tempdir().unwrap();
    let ghost = dir.path().join("ghost.txt");

    let result = run(fs_read_file_range_impl(
        ghost.to_string_lossy().to_string(),
        0,
        10,
        Some(dir.path().to_path_buf()),
    ));
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains(&ghost.to_string_lossy().to_string()),
        "impl 层不存在路径错误应含路径, 实际: {err}"
    );
}
