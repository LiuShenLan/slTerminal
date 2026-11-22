//! Markdown 安全资源的解码与本地授权边界。

mod decoder;
mod formula;
mod link;
mod local_resource;
mod remote_resource;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkdownResourceError {
    Unauthorized,
    Read,
    Changed,
    TooLarge,
    Unsafe,
    Decode,
}
