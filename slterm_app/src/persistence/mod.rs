//! SQLite 持久化适配；生产调用只经专用后台所有者。

pub(crate) mod database;
pub(crate) mod journal;
pub(crate) mod learning;
pub(crate) mod operation;
pub(crate) mod workspace;
