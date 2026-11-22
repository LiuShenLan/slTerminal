//! 远程与内联 Markdown 图片的单次授权及共享缓存。

use std::io::Read as _;
use std::sync::Arc;
use std::time::Duration;

use data_url::DataUrl;
use sha2::{Digest as _, Sha256};
use slterm_document::DocumentId;

use super::MarkdownResourceError;
use super::decoder::MarkdownImageKind;
use super::local_resource::{LoadedMarkdownImage, MarkdownImageService, MarkdownResourceKey};

const MAX_RESOURCE_BYTES: usize = 20 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RemoteImageKey {
    document: DocumentId,
    url: String,
    generation: u64,
}

impl RemoteImageKey {
    fn new(document: DocumentId, url: &str, generation: u64) -> Self {
        Self {
            document,
            url: url.to_owned(),
            generation,
        }
    }
}

pub(crate) struct RemoteImageResponse {
    body: Vec<u8>,
    mime: String,
}

pub(crate) trait RemoteImageFetch: Send + Sync {
    fn fetch(&self, url: &str) -> Result<RemoteImageResponse, MarkdownResourceError>;
}

pub(crate) struct UreqRemoteImageClient {
    agent: ureq::Agent,
}

impl UreqRemoteImageClient {
    pub(crate) fn new() -> Self {
        let agent = ureq::config::Config::builder()
            .proxy(None)
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .new_agent();
        Self { agent }
    }
}

impl RemoteImageFetch for UreqRemoteImageClient {
    fn fetch(&self, url: &str) -> Result<RemoteImageResponse, MarkdownResourceError> {
        validate_remote_url(url)?;
        let mut response = self
            .agent
            .get(url)
            .header("Accept", "image/*")
            .call()
            .map_err(|_| MarkdownResourceError::Read)?;
        if response.status() != 200 {
            return Err(MarkdownResourceError::Read);
        }
        let mime = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_RESOURCE_BYTES as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| MarkdownResourceError::Read)?;
        if body.len() > MAX_RESOURCE_BYTES {
            return Err(MarkdownResourceError::TooLarge);
        }
        Ok(RemoteImageResponse { body, mime })
    }
}

impl MarkdownImageService {
    pub(crate) fn with_remote_fetch(mut self, remote: Arc<dyn RemoteImageFetch>) -> Self {
        self.remote = remote;
        self
    }

    /// 无点击入口：只读取当前授权键对应的成功缓存，不触发网络。
    pub(crate) fn peek_remote(
        &mut self,
        document: DocumentId,
        url: &str,
        generation: u64,
    ) -> Option<LoadedMarkdownImage> {
        let authorization = RemoteImageKey::new(document, url, generation);
        let key = self.remote_cache.get(&authorization).cloned()?;
        if let Some(pixels) = self.cache.get(&key) {
            return Some(LoadedMarkdownImage::new(key, pixels));
        }
        self.remote_cache.remove(&authorization);
        None
    }

    /// 用户点击单图后的唯一网络入口；失败不缓存，后续点击可重试。
    pub(crate) fn fetch_remote_on_click(
        &mut self,
        document: DocumentId,
        url: &str,
        generation: u64,
    ) -> Result<LoadedMarkdownImage, MarkdownResourceError> {
        validate_remote_url(url)?;
        if let Some(loaded) = self.peek_remote(document, url, generation) {
            return Ok(loaded);
        }
        let authorization = RemoteImageKey::new(document, url, generation);
        let response = self.remote.fetch(url)?;
        let kind = image_kind_for_mime(&response.mime)?;
        let pixels = Arc::new(self.decoder.decode(&response.body, kind)?);
        let key = MarkdownResourceKey::Remote {
            authorization: authorization.clone(),
            content_hash: Sha256::digest(&response.body).into(),
        };
        self.cache.insert(key.clone(), pixels.clone());
        if self.cache.contains(&key) {
            self.remote_cache.insert(authorization, key.clone());
        }
        self.prune_remote_index();
        Ok(LoadedMarkdownImage::new(key, pixels))
    }

    pub(crate) fn load_data_image(
        &mut self,
        url: &str,
    ) -> Result<LoadedMarkdownImage, MarkdownResourceError> {
        let data = DataUrl::process(url).map_err(|_| MarkdownResourceError::Unsafe)?;
        if data.mime_type().type_ != "image" {
            return Err(MarkdownResourceError::Unsafe);
        }
        let kind = image_kind_for_mime(&format!(
            "{}/{}",
            data.mime_type().type_,
            data.mime_type().subtype
        ))?;
        let mut body = Vec::new();
        let mut failure = MarkdownResourceError::Decode;
        data.decode(|chunk| {
            if body.len().saturating_add(chunk.len()) > MAX_RESOURCE_BYTES {
                failure = MarkdownResourceError::TooLarge;
                return Err(());
            }
            body.extend_from_slice(chunk);
            Ok(())
        })
        .map_err(|_| failure)?;
        let key = MarkdownResourceKey::Data {
            content_hash: Sha256::digest(&body).into(),
        };
        if let Some(pixels) = self.cache.get(&key) {
            return Ok(LoadedMarkdownImage::new(key, pixels));
        }
        let pixels = Arc::new(self.decoder.decode(&body, kind)?);
        self.cache.insert(key.clone(), pixels.clone());
        Ok(LoadedMarkdownImage::new(key, pixels))
    }

    fn prune_remote_index(&mut self) {
        let stale = self
            .remote_cache
            .iter()
            .filter(|(_, key)| !self.cache.contains(key))
            .map(|(authorization, _)| authorization.clone())
            .collect::<Vec<_>>();
        for authorization in stale {
            self.remote_cache.remove(&authorization);
        }
    }
}

fn image_kind_for_mime(mime: &str) -> Result<MarkdownImageKind, MarkdownResourceError> {
    let mime = mime.split(';').next().unwrap_or_default().trim();
    let Some(subtype) = mime.strip_prefix("image/") else {
        return Err(MarkdownResourceError::Unsafe);
    };
    if subtype.is_empty() {
        return Err(MarkdownResourceError::Unsafe);
    }
    Ok(if subtype.eq_ignore_ascii_case("svg+xml") {
        MarkdownImageKind::Svg
    } else {
        MarkdownImageKind::Raster
    })
}

pub(super) fn validate_remote_url(url: &str) -> Result<(), MarkdownResourceError> {
    if url.is_empty()
        || url.trim() != url
        || url.chars().any(char::is_control)
        || url.contains('\\')
    {
        return Err(MarkdownResourceError::Unsafe);
    }
    let Some(colon) = url.find(':') else {
        return Err(MarkdownResourceError::Unsafe);
    };
    if !matches!(url[..colon].to_ascii_lowercase().as_str(), "http" | "https") {
        return Err(MarkdownResourceError::Unsafe);
    }
    let authority = url[colon + 1..]
        .strip_prefix("//")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return Err(MarkdownResourceError::Unsafe);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use slterm_document::DocumentId;
    use uuid::Uuid;

    use super::*;
    use crate::app_shell::markdown::decoder::BoundedMarkdownImageDecoder;
    use crate::app_shell::markdown::local_resource::MarkdownImageService;
    use crate::platform::document_paths::SystemDocumentPathResolver;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="#ff0000"/></svg>"##;

    struct FakeRemote {
        calls: Arc<Mutex<Vec<String>>>,
        results: Mutex<VecDeque<Result<RemoteImageResponse, MarkdownResourceError>>>,
    }

    impl FakeRemote {
        fn successful(calls: Arc<Mutex<Vec<String>>>) -> Self {
            Self {
                calls,
                results: Mutex::new(VecDeque::new()),
            }
        }
    }

    impl RemoteImageFetch for FakeRemote {
        fn fetch(&self, url: &str) -> Result<RemoteImageResponse, MarkdownResourceError> {
            self.calls.lock().unwrap().push(url.to_owned());
            self.results.lock().unwrap().pop_front().unwrap_or_else(|| {
                Ok(RemoteImageResponse {
                    body: SVG.as_bytes().to_vec(),
                    mime: "image/svg+xml".to_owned(),
                })
            })
        }
    }

    fn new_document() -> DocumentId {
        DocumentId::from_uuid(Uuid::now_v7()).unwrap()
    }

    fn service(remote: Arc<dyn RemoteImageFetch>, cache_bytes: usize) -> MarkdownImageService {
        MarkdownImageService::with_dependencies(
            Arc::new(SystemDocumentPathResolver),
            Arc::new(BoundedMarkdownImageDecoder),
            cache_bytes,
        )
        .with_remote_fetch(remote)
    }

    #[test]
    fn markdown_remote_image_click_authorization() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut service = service(Arc::new(FakeRemote::successful(calls.clone())), 128);
        let document = new_document();
        let selected = "https://images.example/selected.svg?exact=1";
        let other_candidate = "https://images.example/other.svg";
        assert!(service.peek_remote(document, selected, 0).is_none());
        assert!(calls.lock().unwrap().is_empty());

        let loaded = service
            .fetch_remote_on_click(document, selected, 0)
            .unwrap();
        assert_eq!(calls.lock().unwrap().as_slice(), [selected]);
        let cached = service.peek_remote(document, selected, 0).unwrap();
        assert!(Arc::ptr_eq(loaded.pixels(), cached.pixels()));
        service
            .fetch_remote_on_click(document, selected, 0)
            .unwrap();
        assert_eq!(calls.lock().unwrap().len(), 1);

        assert!(service.peek_remote(document, other_candidate, 0).is_none());
        service
            .fetch_remote_on_click(document, other_candidate, 0)
            .unwrap();
        assert!(service.peek_remote(document, selected, 1).is_none());
        service
            .fetch_remote_on_click(document, selected, 1)
            .unwrap();
        let other_document = new_document();
        assert!(service.peek_remote(other_document, selected, 0).is_none());
        service
            .fetch_remote_on_click(other_document, selected, 0)
            .unwrap();
        assert_eq!(calls.lock().unwrap().len(), 4);
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            [selected, other_candidate, selected, selected]
        );
    }

    #[test]
    fn remote_failure_retry_and_eviction_require_explicit_clicks() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let remote = FakeRemote::successful(calls.clone());
        remote
            .results
            .lock()
            .unwrap()
            .push_back(Err(MarkdownResourceError::Read));
        let mut service = service(Arc::new(remote), 4);
        let document = new_document();
        let first = "https://images.example/first.svg";
        let second = "https://images.example/second.svg";
        assert!(matches!(
            service.fetch_remote_on_click(document, first, 0),
            Err(MarkdownResourceError::Read)
        ));
        assert!(service.peek_remote(document, first, 0).is_none());
        service.fetch_remote_on_click(document, first, 0).unwrap();
        assert_eq!(calls.lock().unwrap().len(), 2);
        service.fetch_remote_on_click(document, second, 0).unwrap();
        assert!(service.peek_remote(document, first, 0).is_none());
        service.fetch_remote_on_click(document, first, 0).unwrap();
        assert_eq!(calls.lock().unwrap().len(), 4);
    }

    #[test]
    fn remote_url_shape_is_validated_before_fetch() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut service = service(Arc::new(FakeRemote::successful(calls.clone())), 128);
        let document = new_document();
        for url in [
            "javascript:alert(1)",
            "file:///D:/secret.png",
            "data:image/png;base64,AAAA",
            "https://user:password@example.com/image.png",
            " https://example.com/image.png",
            "https:///missing-host",
        ] {
            assert!(matches!(
                service.fetch_remote_on_click(document, url, 0),
                Err(MarkdownResourceError::Unsafe)
            ));
        }
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn data_images_decode_without_network_and_reject_other_media() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut service = service(Arc::new(FakeRemote::successful(calls)), 128);
        let encoded = format!(
            "data:image/svg+xml;base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, SVG)
        );
        let decoded = service.load_data_image(&encoded).unwrap();
        assert_eq!(decoded.pixels().dimensions(), (1, 1));
        let again = service.load_data_image(&encoded).unwrap();
        assert!(Arc::ptr_eq(decoded.pixels(), again.pixels()));
        assert!(matches!(
            service.load_data_image("data:text/plain;base64,QQ=="),
            Err(MarkdownResourceError::Unsafe)
        ));
        assert!(
            service
                .load_data_image("data:image/svg+xml,not-closed")
                .is_err()
        );
    }
}

#[cfg(test)]
mod ureq_tests {
    use std::io::{BufRead as _, BufReader, Write as _};
    use std::net::{SocketAddr, TcpListener};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use super::*;

    struct HttpFixture {
        address: SocketAddr,
        worker: JoinHandle<Vec<String>>,
    }

    impl HttpFixture {
        fn start(responses: Vec<Vec<u8>>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let worker = thread::spawn(move || {
                let mut requests = Vec::new();
                'responses: for response in responses {
                    let deadline = Instant::now() + Duration::from_millis(300);
                    let stream = loop {
                        match listener.accept() {
                            Ok((stream, _)) => break stream,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                if Instant::now() >= deadline {
                                    break 'responses;
                                }
                                thread::sleep(Duration::from_millis(2));
                            }
                            Err(error) => panic!("HTTP fixture accept failed: {error}"),
                        }
                    };
                    stream.set_nonblocking(false).unwrap();
                    let mut reader = BufReader::new(stream);
                    let mut request = String::new();
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        if line == "\r\n" {
                            break;
                        }
                        request.push_str(&line);
                    }
                    requests.push(request);
                    reader.get_mut().write_all(&response).unwrap();
                }
                requests
            });
            Self { address, worker }
        }

        fn url(&self) -> String {
            format!("http://{}/image.svg", self.address)
        }

        fn finish(self) -> Vec<String> {
            self.worker.join().unwrap()
        }
    }

    fn response(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    #[test]
    fn ureq_remote_request_has_no_credentials_or_implicit_headers() {
        let fixture = HttpFixture::start(vec![response(
            "200 OK",
            "Content-Type: image/svg+xml\r\n",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"##.as_bytes(),
        )]);
        let client = UreqRemoteImageClient::new();
        assert_eq!(client.agent.config().max_redirects(), 0);
        assert_eq!(
            client.agent.config().timeouts().global,
            Some(Duration::from_secs(10))
        );
        assert!(client.agent.config().proxy().is_none());
        let response = client.fetch(&fixture.url()).unwrap();
        assert_eq!(response.mime, "image/svg+xml");
        let requests = fixture.finish();
        assert_eq!(requests.len(), 1);
        let request = requests[0].to_ascii_lowercase();
        assert!(request.starts_with("get /image.svg http/1.1\r\n"));
        for forbidden in [
            "cookie:",
            "authorization:",
            "proxy-authorization:",
            "referer:",
        ] {
            assert!(!request.contains(forbidden), "请求不得包含 {forbidden}");
        }
    }

    #[test]
    fn ureq_remote_redirect_is_returned_without_second_request() {
        let fixture = HttpFixture::start(vec![
            response(
                "302 Found",
                "Location: http://127.0.0.1/redirected.svg\r\n",
                b"",
            ),
            response("200 OK", "Content-Type: image/svg+xml\r\n", b"unexpected"),
        ]);
        let client = UreqRemoteImageClient::new();
        assert!(matches!(
            client.fetch(&fixture.url()),
            Err(MarkdownResourceError::Read)
        ));
        assert_eq!(fixture.finish().len(), 1);
    }

    #[test]
    fn ureq_remote_body_is_capped_at_twenty_mib() {
        let body = vec![0; 20 * 1024 * 1024 + 1];
        let fixture = HttpFixture::start(vec![response(
            "200 OK",
            "Content-Type: image/png\r\n",
            &body,
        )]);
        let client = UreqRemoteImageClient::new();
        assert!(matches!(
            client.fetch(&fixture.url()),
            Err(MarkdownResourceError::TooLarge)
        ));
        assert_eq!(fixture.finish().len(), 1);
    }
}
