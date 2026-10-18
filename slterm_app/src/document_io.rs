//! 有界前缀读取(07 篇):内存上限先于解码/解析生效。
//!
//! 读取上限 + 1 字节只为探到溢出事实,不把整个文件物化;溢出且末尾落在
//! UTF-8 多字节中缝时回退到完整字符边界,后续 decode 不会看到半个字符。

use std::io::{self, Read};
use std::path::Path;

pub(crate) fn read_prefix(
    reader: &mut impl Read,
    declared_size: u64,
    limit: usize,
) -> io::Result<(Vec<u8>, bool)> {
    let capacity = declared_size.min(limit as u64) as usize;
    let mut bytes = Vec::with_capacity(capacity);
    reader.by_ref().take(limit as u64).read_to_end(&mut bytes)?;
    // 单独探一个字节判断溢出,避免为确认截断把 8 MiB 的 Vec 再翻倍。
    let mut extra = [0u8; 1];
    let truncated = reader.read(&mut extra)? != 0;
    if truncated
        && let Err(error) = std::str::from_utf8(&bytes)
        && error.error_len().is_none()
    {
        bytes.truncate(error.valid_up_to());
    }
    Ok((bytes, truncated))
}

/// 本地文件的有界前缀读取(文档/源码 tab 的载入入口)。
pub(crate) fn read_path_prefix(path: &Path, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut file = std::fs::File::open(path)?;
    let declared_size = file.metadata()?.len();
    read_prefix(&mut file, declared_size, limit)
}

#[cfg(test)]
mod document_io_tests {
    use super::*;

    struct VirtualLargeFile {
        remaining: usize,
        read: usize,
    }

    impl Read for VirtualLargeFile {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let count = buffer.len().min(self.remaining);
            buffer[..count].fill(b'x');
            self.remaining -= count;
            self.read += count;
            Ok(count)
        }
    }

    /// 100 MiB 虚拟文件只读 limit + 1 字节,内存从未物化全文。
    #[test]
    fn virtual_large_file_never_materializes_its_full_contents() {
        let length = 100 * 1024 * 1024;
        let limit = 64 * 1024;
        let mut file = VirtualLargeFile {
            remaining: length,
            read: 0,
        };
        let (bytes, truncated) = read_prefix(&mut file, length as u64, limit).unwrap();
        assert!(truncated);
        assert_eq!(file.read, limit + 1);
        assert_eq!(bytes.len(), limit);
        assert_eq!(bytes.capacity(), limit);
    }

    /// 短文件、Unicode 中缝截断与恰好等于上限三种边界。
    #[test]
    fn short_files_growth_and_unicode_boundaries_remain_correct() {
        let (bytes, truncated) = read_prefix(&mut "甲乙丙".as_bytes(), 9, 7).unwrap();
        assert!(truncated);
        assert_eq!(std::str::from_utf8(&bytes).unwrap(), "甲乙");
        let (bytes, truncated) = read_prefix(&mut &b"abcdef"[..], 1, 4).unwrap();
        assert!(truncated);
        assert_eq!(bytes, b"abcd");
        let (bytes, truncated) = read_prefix(&mut &b"abcd"[..], 4, 4).unwrap();
        assert!(!truncated);
        assert_eq!(bytes, b"abcd");
    }

    /// 路径入口与 reader 入口同语义(临时文件隔离)。
    #[test]
    fn path_reader_bounds_and_reports_truncation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.txt");
        std::fs::write(&path, b"abcdef").unwrap();
        let (bytes, truncated) = read_path_prefix(&path, 4).unwrap();
        assert_eq!(bytes, b"abcd");
        assert!(truncated);
        let (bytes, truncated) = read_path_prefix(&path, 8).unwrap();
        assert_eq!(bytes, b"abcdef");
        assert!(!truncated);
    }
}
