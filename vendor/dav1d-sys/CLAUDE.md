# dav1d-sys 仓内补丁

> 上游精确版本源码副本，仅修正 Windows MSVC 构建环境。

## 决策与红线

- **上游锚**：`dav1d-sys 0.8.3`，对应 `rust-av/dav1d-rs` commit `9f62f346c6841d1af4ab3eb8267c73041c8c4ee6`。升级必须同步完整源码与许可，不保留无审查的差异。
- **补丁边界**：只在 Meson setup 增加 `--vsenv`，防止 PATH 中的 MinGW 与 MSVC Rust 目标混链；FFI、crate 版本和 dav1d 上游版本不变。
- **许可**：保留上游 MIT `LICENSE`；dav1d 原生 BSD 文本继续经生产许可生成器收录，仓内路径包不得被排除。

## 验收与测试

- Windows x64 构建日志必须确认 C 编译器为 MSVC，AVIF 真实样本解码通过。
- 产品构建后的 `dumpbin /dependents` 不得出现 dav1d DLL。
- `scripts/check_licenses.py` 必须同时收录仓内 binding 与原生源许可。
