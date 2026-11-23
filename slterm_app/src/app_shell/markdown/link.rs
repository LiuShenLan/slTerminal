//! Markdown 链接的锚点、系统处理程序与本地文件分派。

use std::sync::Arc;

use slterm_document::{BlockId, MarkdownLink};
use slterm_path::{PathGrant, ResolvedPath};

use super::MarkdownResourceError;
use super::local_resource::LocalMarkdownImageRequest;
use super::local_resource::url_scheme;
use super::remote_resource::validate_remote_url;
use crate::app_shell::documents::{DocumentOpenError, DocumentOpenService};

pub(crate) trait SystemLinkOpen: Send + Sync {
    fn open(&self, url: &str) -> Result<(), MarkdownResourceError>;
}

struct SystemLinkOpener;

impl SystemLinkOpen for SystemLinkOpener {
    fn open(&self, url: &str) -> Result<(), MarkdownResourceError> {
        crate::platform::file_operations::open_url_in_system(url)
            .map_err(|_| MarkdownResourceError::Read)
    }
}

pub(crate) enum MarkdownLinkOutcome {
    Anchor(Option<BlockId>),
    System,
    Local {
        document: crate::app_shell::documents::ResolvedDocument,
        fragment: Option<String>,
    },
    Rejected,
}

pub(crate) struct MarkdownLinkDispatcher {
    documents: Arc<DocumentOpenService>,
    opener: Arc<dyn SystemLinkOpen>,
}

impl Default for MarkdownLinkDispatcher {
    fn default() -> Self {
        Self::new(
            Arc::new(DocumentOpenService::default()),
            Arc::new(SystemLinkOpener),
        )
    }
}

impl MarkdownLinkDispatcher {
    pub(crate) fn new(
        documents: Arc<DocumentOpenService>,
        opener: Arc<dyn SystemLinkOpen>,
    ) -> Self {
        Self { documents, opener }
    }

    /// 本地解析与系统打开均只允许在后台执行体调用。
    pub(crate) fn dispatch(
        &self,
        document: &ResolvedPath,
        link: &MarkdownLink,
        persistent_grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<MarkdownLinkOutcome, MarkdownResourceError> {
        let url = link.url();
        if url.starts_with('#') {
            return Ok(if url == "#" {
                MarkdownLinkOutcome::Anchor(None)
            } else if let Some(target) = link.target() {
                MarkdownLinkOutcome::Anchor(Some(target))
            } else {
                MarkdownLinkOutcome::Rejected
            });
        }
        if let Some(scheme) = url_scheme(url) {
            let supported = match scheme.to_ascii_lowercase().as_str() {
                "http" | "https" => validate_remote_url(url).is_ok(),
                "mailto" | "tel" => valid_handler_url(url),
                _ => false,
            };
            if !supported {
                return Ok(MarkdownLinkOutcome::Rejected);
            }
            self.opener.open(url)?;
            return Ok(MarkdownLinkOutcome::System);
        }
        let Ok(path) = LocalMarkdownImageRequest::path_from_markdown(document, url) else {
            return Ok(MarkdownLinkOutcome::Rejected);
        };
        let resolved = self
            .documents
            .resolve_link(&path, persistent_grants, authorization_generation)
            .map_err(|error| match error {
                DocumentOpenError::Unauthorized => MarkdownResourceError::Unauthorized,
                DocumentOpenError::Io(_) => MarkdownResourceError::Read,
            })?;
        Ok(MarkdownLinkOutcome::Local {
            document: resolved,
            fragment: url
                .split_once('#')
                .map(|(_, fragment)| fragment.to_owned())
                .filter(|fragment| !fragment.is_empty()),
        })
    }
}

fn valid_handler_url(url: &str) -> bool {
    !url.is_empty()
        && url.trim() == url
        && !url.chars().any(char::is_control)
        && !url.contains('\\')
        && url
            .split_once(':')
            .is_some_and(|(_, target)| !target.is_empty())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use slterm_document::MarkdownDocument;
    use slterm_path::{Capabilities, GrantPersistence};
    use tempfile::TempDir;

    use super::*;
    use crate::app_shell::documents::DocumentOpenService;
    use crate::platform::document_paths::{DocumentPathResolver as _, SystemDocumentPathResolver};

    struct RecordingOpener {
        urls: Arc<Mutex<Vec<String>>>,
    }

    impl SystemLinkOpen for RecordingOpener {
        fn open(&self, url: &str) -> Result<(), MarkdownResourceError> {
            self.urls.lock().unwrap().push(url.to_owned());
            Ok(())
        }
    }

    fn canonical(path: &std::path::Path) -> slterm_path::CanonicalPath {
        slterm_path::CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap()
    }

    #[test]
    fn markdown_links_dispatch_anchors_system_handlers_and_dangerous_schemes() {
        let directory = TempDir::new().unwrap();
        let document_path = directory.path().join("document.md");
        std::fs::write(&document_path, "# Heading\n").unwrap();
        let document_location = SystemDocumentPathResolver
            .resolve(&canonical(&document_path))
            .unwrap();
        let document = MarkdownDocument::parse(
            "# Heading\n\n[anchor](#heading) [top](#) [web](https://example.com/a) [mail](mailto:user@example.com) [phone](tel:+123) [bad](javascript:alert(1)) [file](file:///D:/secret)\n",
        );
        let urls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = MarkdownLinkDispatcher::new(
            Arc::new(DocumentOpenService::default()),
            Arc::new(RecordingOpener { urls: urls.clone() }),
        );
        let links = document.links();
        let heading = document.headings()[0].block();
        assert!(matches!(
            dispatcher.dispatch(&document_location, &links[0], &[], 0).unwrap(),
            MarkdownLinkOutcome::Anchor(Some(block)) if block == heading
        ));
        assert!(matches!(
            dispatcher
                .dispatch(&document_location, &links[1], &[], 0)
                .unwrap(),
            MarkdownLinkOutcome::Anchor(None)
        ));
        for link in &links[2..5] {
            assert!(matches!(
                dispatcher
                    .dispatch(&document_location, link, &[], 0)
                    .unwrap(),
                MarkdownLinkOutcome::System
            ));
        }
        assert_eq!(
            urls.lock().unwrap().as_slice(),
            [
                "https://example.com/a",
                "mailto:user@example.com",
                "tel:+123"
            ]
        );
        for link in &links[5..] {
            assert!(matches!(
                dispatcher
                    .dispatch(&document_location, link, &[], 0)
                    .unwrap(),
                MarkdownLinkOutcome::Rejected
            ));
        }
        assert_eq!(urls.lock().unwrap().len(), 3);
    }

    #[test]
    fn outside_markdown_link_click_grants_only_exact_session_read() {
        let directory = TempDir::new().unwrap();
        let document_path = directory.path().join("outside.md");
        let target = directory.path().join("target.md");
        std::fs::write(&document_path, "[target](target.md)").unwrap();
        std::fs::write(&target, "# Target\n").unwrap();
        let document_location = SystemDocumentPathResolver
            .resolve(&canonical(&document_path))
            .unwrap();
        let document = MarkdownDocument::parse("[target](target.md)");
        let dispatcher = MarkdownLinkDispatcher::new(
            Arc::new(DocumentOpenService::default()),
            Arc::new(RecordingOpener {
                urls: Arc::new(Mutex::new(Vec::new())),
            }),
        );
        let outcome = dispatcher
            .dispatch(&document_location, &document.links()[0], &[], 0)
            .unwrap();
        let MarkdownLinkOutcome::Local {
            document: resolved, ..
        } = outcome
        else {
            panic!("本地链接必须产生统一文件打开候选")
        };
        assert_eq!(resolved.location().canonical_path(), &canonical(&target));
        assert_eq!(resolved.grants().len(), 1);
        let grant = &resolved.grants()[0];
        assert_eq!(grant.persistence(), GrantPersistence::Session);
        assert_eq!(grant.path(), &canonical(&target));
        assert_eq!(grant.capabilities(), Capabilities::READ);
    }

    #[test]
    fn missing_anchor_is_rejected_without_opening_anything() {
        let directory = TempDir::new().unwrap();
        let document_path = directory.path().join("document.md");
        std::fs::write(&document_path, "[missing](#missing)").unwrap();
        let document_location = SystemDocumentPathResolver
            .resolve(&canonical(&document_path))
            .unwrap();
        let document = MarkdownDocument::parse("[missing](#missing)");
        let urls = Arc::new(Mutex::new(Vec::new()));
        let dispatcher = MarkdownLinkDispatcher::new(
            Arc::new(DocumentOpenService::default()),
            Arc::new(RecordingOpener { urls: urls.clone() }),
        );
        assert!(matches!(
            dispatcher
                .dispatch(&document_location, &document.links()[0], &[], 0)
                .unwrap(),
            MarkdownLinkOutcome::Rejected
        ));
        assert!(urls.lock().unwrap().is_empty());
    }
}
