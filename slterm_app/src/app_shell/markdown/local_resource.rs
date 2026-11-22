//! 本地 Markdown 图片的逐资源授权、身份复核与解码 LRU。

use std::collections::HashMap;
use std::fs::File;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use image::RgbaImage;
use sha2::{Digest as _, Sha256};
use slterm_path::{
    CanonicalPath, FileIdentity, PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath,
};

use super::MarkdownResourceError;
use super::decoder::{BoundedMarkdownImageDecoder, MarkdownImageKind};
use crate::platform::document_paths::{DocumentPathResolver, SystemDocumentPathResolver};

const MAX_RESOURCE_BYTES: usize = 20 * 1024 * 1024;
const IMAGE_CACHE_BYTES: usize = 128 * 1024 * 1024;

pub(crate) trait MarkdownImageDecode: Send + Sync {
    fn decode(
        &self,
        bytes: &[u8],
        kind: MarkdownImageKind,
    ) -> Result<RgbaImage, MarkdownResourceError>;
}

impl MarkdownImageDecode for BoundedMarkdownImageDecoder {
    fn decode(
        &self,
        bytes: &[u8],
        kind: MarkdownImageKind,
    ) -> Result<RgbaImage, MarkdownResourceError> {
        Self::decode(bytes, kind)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LocalMarkdownImageRequest {
    lexical_path: CanonicalPath,
    session_grant: Option<PathGrant>,
    expected_authorization_generation: u64,
}

impl LocalMarkdownImageRequest {
    pub(crate) fn new(lexical_path: CanonicalPath, authorization_generation: u64) -> Self {
        Self {
            lexical_path,
            session_grant: None,
            expected_authorization_generation: authorization_generation,
        }
    }

    pub(crate) fn from_markdown(
        document: &ResolvedPath,
        url: &str,
        authorization_generation: u64,
    ) -> Result<Self, MarkdownResourceError> {
        let path = url
            .split(['#', '?'])
            .next()
            .filter(|path| !path.is_empty())
            .ok_or(MarkdownResourceError::Unsafe)?;
        if has_url_scheme(path) {
            return Err(MarkdownResourceError::Unsafe);
        }
        let resource = Path::new(path);
        let lexical = if resource.is_absolute() {
            resource.to_path_buf()
        } else {
            document
                .canonical_path()
                .as_path()
                .parent()
                .ok_or(MarkdownResourceError::Unsafe)?
                .join(resource)
        };
        Ok(Self::new(
            normalize_resource_path(&lexical)?,
            authorization_generation,
        ))
    }

    pub(crate) fn with_session_grant(mut self, grant: PathGrant) -> Self {
        self.session_grant = Some(grant);
        self
    }
}

fn has_url_scheme(path: &str) -> bool {
    let Some(colon) = path.find(':') else {
        return false;
    };
    let windows_drive = colon == 1
        && path.as_bytes()[0].is_ascii_alphabetic()
        && matches!(path.as_bytes().get(2), Some(b'/' | b'\\'));
    if windows_drive {
        return false;
    }
    let separator = path.find(['/', '\\']).unwrap_or(path.len());
    colon < separator
        && path.as_bytes()[0].is_ascii_alphabetic()
        && path[..colon]
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn normalize_resource_path(path: &Path) -> Result<CanonicalPath, MarkdownResourceError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::RootDir) => {}
                _ => return Err(MarkdownResourceError::Unsafe),
            },
        }
    }
    CanonicalPath::new(normalized).map_err(|_| MarkdownResourceError::Unsafe)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MarkdownResourceKey {
    location: CanonicalPath,
    identity: FileIdentity,
    content_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub(crate) struct LoadedMarkdownImage {
    key: MarkdownResourceKey,
    pixels: Arc<RgbaImage>,
}

impl LoadedMarkdownImage {
    pub(crate) fn key(&self) -> &MarkdownResourceKey {
        &self.key
    }

    pub(crate) fn pixels(&self) -> &Arc<RgbaImage> {
        &self.pixels
    }
}

pub(crate) struct LocalMarkdownImageService {
    resolver: Arc<dyn DocumentPathResolver>,
    decoder: Arc<dyn MarkdownImageDecode>,
    cache: RasterCache,
}

impl Default for LocalMarkdownImageService {
    fn default() -> Self {
        Self::with_dependencies(
            Arc::new(SystemDocumentPathResolver),
            Arc::new(BoundedMarkdownImageDecoder),
            IMAGE_CACHE_BYTES,
        )
    }
}

impl LocalMarkdownImageService {
    pub(crate) fn with_dependencies(
        resolver: Arc<dyn DocumentPathResolver>,
        decoder: Arc<dyn MarkdownImageDecode>,
        cache_bytes: usize,
    ) -> Self {
        Self {
            resolver,
            decoder,
            cache: RasterCache::new(cache_bytes),
        }
    }

    /// 只在后台调用；命中缓存前仍重新解析、授权并核对当前文件内容。
    pub(crate) fn load(
        &mut self,
        request: &LocalMarkdownImageRequest,
        persistent_grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<LoadedMarkdownImage, MarkdownResourceError> {
        let resolved = self
            .resolver
            .resolve(&request.lexical_path)
            .map_err(|_| MarkdownResourceError::Read)?;
        let path_request = PathRequest {
            operation: PathOperation::ResourceRead,
            lexical_path: request.lexical_path.as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant: request.session_grant.clone(),
            expected_authorization_generation: request.expected_authorization_generation,
        };
        let authorization = PathPolicy::authorize(
            &path_request,
            &resolved,
            persistent_grants,
            current_authorization_generation,
        )
        .map_err(|_| MarkdownResourceError::Unauthorized)?;
        let location = authorization.resolved_path();
        let expected_identity = location
            .file_identity()
            .ok_or(MarkdownResourceError::Changed)?;
        let mut file = File::open(location.canonical_path().as_path())
            .map_err(|_| MarkdownResourceError::Read)?;
        let actual_identity =
            crate::atomic_file::file_identity(&file).map_err(|_| MarkdownResourceError::Read)?;
        if actual_identity != expected_identity {
            return Err(MarkdownResourceError::Changed);
        }
        let metadata = file.metadata().map_err(|_| MarkdownResourceError::Read)?;
        if !metadata.is_file() {
            return Err(MarkdownResourceError::Read);
        }
        if metadata.len() > MAX_RESOURCE_BYTES as u64 {
            return Err(MarkdownResourceError::TooLarge);
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        (&mut file)
            .take(MAX_RESOURCE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| MarkdownResourceError::Read)?;
        if bytes.len() > MAX_RESOURCE_BYTES {
            return Err(MarkdownResourceError::TooLarge);
        }
        let final_metadata = file.metadata().map_err(|_| MarkdownResourceError::Read)?;
        if bytes.len() as u64 != metadata.len()
            || final_metadata.len() != metadata.len()
            || final_metadata.modified().ok() != metadata.modified().ok()
        {
            return Err(MarkdownResourceError::Changed);
        }

        let key = MarkdownResourceKey {
            location: location.canonical_path().clone(),
            identity: actual_identity,
            content_hash: Sha256::digest(&bytes).into(),
        };
        if let Some(pixels) = self.cache.get(&key) {
            return Ok(LoadedMarkdownImage { key, pixels });
        }
        let kind = if location
            .canonical_path()
            .as_path()
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
        {
            MarkdownImageKind::Svg
        } else {
            MarkdownImageKind::Raster
        };
        let pixels = Arc::new(self.decoder.decode(&bytes, kind)?);
        self.cache.insert(key.clone(), pixels.clone());
        Ok(LoadedMarkdownImage { key, pixels })
    }

    #[cfg(test)]
    fn cached_entries(&self) -> usize {
        self.cache.entries.len()
    }

    #[cfg(test)]
    fn cached_bytes(&self) -> usize {
        self.cache.used
    }
}

struct RasterCache {
    entries: HashMap<MarkdownResourceKey, RasterEntry>,
    limit: usize,
    used: usize,
    clock: u64,
}

struct RasterEntry {
    pixels: Arc<RgbaImage>,
    bytes: usize,
    touched: u64,
}

impl RasterCache {
    fn new(limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            limit,
            used: 0,
            clock: 0,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock = self.clock.saturating_add(1);
        self.clock
    }

    fn get(&mut self, key: &MarkdownResourceKey) -> Option<Arc<RgbaImage>> {
        let tick = self.tick();
        let entry = self.entries.get_mut(key)?;
        entry.touched = tick;
        Some(entry.pixels.clone())
    }

    fn remove(&mut self, key: &MarkdownResourceKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.used -= entry.bytes;
        }
    }

    fn insert(&mut self, key: MarkdownResourceKey, pixels: Arc<RgbaImage>) {
        self.remove(&key);
        let bytes = pixels.as_raw().len();
        if bytes > self.limit {
            return;
        }
        while self.used.saturating_add(bytes) > self.limit {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| key.clone());
            let Some(oldest) = oldest else { break };
            self.remove(&oldest);
        }
        let tick = self.tick();
        self.used += bytes;
        self.entries.insert(
            key,
            RasterEntry {
                pixels,
                bytes,
                touched: tick,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use slterm_path::{CanonicalPath, PathGrant};
    use tempfile::TempDir;

    use super::*;
    use crate::app_shell::markdown::decoder::MarkdownImageKind;

    struct CountingDecoder {
        calls: Arc<AtomicUsize>,
    }

    impl MarkdownImageDecode for CountingDecoder {
        fn decode(
            &self,
            bytes: &[u8],
            kind: MarkdownImageKind,
        ) -> Result<image::RgbaImage, MarkdownResourceError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            BoundedMarkdownImageDecoder::decode(bytes, kind)
        }
    }

    fn canonical(path: &std::path::Path) -> CanonicalPath {
        CanonicalPath::new(dunce::canonicalize(path).unwrap()).unwrap()
    }

    fn write_png(path: &std::path::Path, color: [u8; 4]) {
        image::RgbaImage::from_pixel(1, 1, image::Rgba(color))
            .save(path)
            .unwrap();
    }

    fn service_with_cache(
        calls: Arc<AtomicUsize>,
        cache_bytes: usize,
    ) -> LocalMarkdownImageService {
        LocalMarkdownImageService::with_dependencies(
            Arc::new(SystemDocumentPathResolver),
            Arc::new(CountingDecoder { calls }),
            cache_bytes,
        )
    }

    #[test]
    fn markdown_outside_document_resource_not_authorized() {
        let directory = TempDir::new().unwrap();
        let project = directory.path().join("project");
        let outside = directory.path().join("outside");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let document = outside.join("document.md");
        let adjacent = outside.join("adjacent.png");
        std::fs::write(&document, "![adjacent](adjacent.png)").unwrap();
        write_png(&adjacent, [1, 2, 3, 255]);
        let inside = project.join("inside.png");
        write_png(&inside, [1, 2, 3, 255]);

        let mut service = service_with_cache(Arc::new(AtomicUsize::new(0)), 128);
        let document_location = SystemDocumentPathResolver
            .resolve(&canonical(&document))
            .unwrap();
        let request =
            LocalMarkdownImageRequest::from_markdown(&document_location, "adjacent.png", 0)
                .unwrap()
                .with_session_grant(PathGrant::session_file_read(canonical(&document)));
        assert_eq!(
            service.load(&request, &[], 0).unwrap_err(),
            MarkdownResourceError::Unauthorized
        );

        let nested = project.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let nested_document = nested.join("document.md");
        std::fs::write(&nested_document, "![inside](../inside.png)").unwrap();
        let nested_location = SystemDocumentPathResolver
            .resolve(&canonical(&nested_document))
            .unwrap();
        let request =
            LocalMarkdownImageRequest::from_markdown(&nested_location, "../inside.png", 0).unwrap();
        let loaded = service
            .load(&request, &[PathGrant::project_root(canonical(&project))], 0)
            .unwrap();
        assert_eq!(loaded.pixels().dimensions(), (1, 1));
        assert!(matches!(
            LocalMarkdownImageRequest::from_markdown(
                &nested_location,
                "https://example.com/remote.png",
                0,
            ),
            Err(MarkdownResourceError::Unsafe)
        ));
    }

    #[test]
    fn local_resource_cache_revalidates_content_identity_and_presence() {
        let directory = TempDir::new().unwrap();
        let image = directory.path().join("image.png");
        write_png(&image, [1, 2, 3, 255]);
        let calls = Arc::new(AtomicUsize::new(0));
        let mut service = service_with_cache(calls.clone(), 128);
        let root = canonical(directory.path());
        let grants = [PathGrant::project_root(root)];
        let request = LocalMarkdownImageRequest::new(canonical(&image), 0);

        let first = service.load(&request, &grants, 0).unwrap();
        let second = service.load(&request, &grants, 0).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(first.key(), second.key());
        assert!(Arc::ptr_eq(first.pixels(), second.pixels()));

        write_png(&image, [9, 8, 7, 255]);
        let changed = service.load(&request, &grants, 0).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_ne!(first.key(), changed.key());
        assert_eq!(changed.pixels().get_pixel(0, 0).0, [9, 8, 7, 255]);

        let replacement = directory.path().join("replacement.png");
        write_png(&replacement, [9, 8, 7, 255]);
        std::fs::remove_file(&image).unwrap();
        std::fs::rename(&replacement, &image).unwrap();
        let replaced = service.load(&request, &grants, 0).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_ne!(changed.key(), replaced.key());

        std::fs::remove_file(&image).unwrap();
        assert_eq!(
            service.load(&request, &grants, 0).unwrap_err(),
            MarkdownResourceError::Read
        );
    }

    #[test]
    fn local_resource_enforces_generation_size_and_lru_budgets() {
        let directory = TempDir::new().unwrap();
        let first = directory.path().join("first.png");
        let second = directory.path().join("second.png");
        write_png(&first, [1, 2, 3, 255]);
        write_png(&second, [4, 5, 6, 255]);
        let grants = [PathGrant::project_root(canonical(directory.path()))];
        let calls = Arc::new(AtomicUsize::new(0));
        let mut service = service_with_cache(calls.clone(), 4);
        let first_request = LocalMarkdownImageRequest::new(canonical(&first), 7);
        let second_request = LocalMarkdownImageRequest::new(canonical(&second), 7);
        assert_eq!(
            service.load(&first_request, &grants, 8).unwrap_err(),
            MarkdownResourceError::Unauthorized
        );
        service.load(&first_request, &grants, 7).unwrap();
        service.load(&second_request, &grants, 7).unwrap();
        assert_eq!(service.cached_entries(), 1);
        assert_eq!(service.cached_bytes(), 4);
        service.load(&first_request, &grants, 7).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        let oversized = directory.path().join("oversized.png");
        std::fs::write(&oversized, vec![0; 20 * 1024 * 1024 + 1]).unwrap();
        let request = LocalMarkdownImageRequest::new(canonical(&oversized), 7);
        assert_eq!(
            service.load(&request, &grants, 7).unwrap_err(),
            MarkdownResourceError::TooLarge
        );
    }
}
