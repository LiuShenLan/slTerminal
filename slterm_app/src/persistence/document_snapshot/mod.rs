//! Document 崩溃快照的自描述信封、文件存储与专用写入队列。

pub(crate) mod envelope;
pub(crate) mod recovery;
pub(crate) mod store;
pub(crate) mod writer;

#[cfg(test)]
mod tests;
