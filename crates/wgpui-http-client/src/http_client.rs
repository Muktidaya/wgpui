//! Executor-neutral HTTP interfaces for WGPUI.

mod async_body;
pub mod github;
pub mod github_download;

pub use anyhow::{Result, anyhow};
pub use async_body::{AsyncBody, Inner};
use derive_more::Deref;
use http::HeaderValue;
pub use http::{self, Method, Request, Response, StatusCode, Uri, request::Builder};

use futures::{
    FutureExt as _,
    future::{self, BoxFuture},
};
use parking_lot::Mutex;
pub use reqwest;
#[cfg(feature = "test-support")]
use std::fmt;
use std::{any::type_name, sync::Arc};
pub use url::Url;

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RedirectPolicy {
    #[default]
    NoFollow,
    FollowLimit(u32),
    FollowAll,
}
pub struct FollowRedirects(pub bool);

pub trait HttpRequestExt {
    /// Conditionally modify self with the given closure.
    fn when(self, condition: bool, then: impl FnOnce(Self) -> Self) -> Self
    where
        Self: Sized,
    {
        if condition { then(self) } else { self }
    }

    /// Conditionally unwrap and modify self with the given closure, if the given option is Some.
    fn when_some<T>(self, option: Option<T>, then: impl FnOnce(Self, T) -> Self) -> Self
    where
        Self: Sized,
    {
        match option {
            Some(value) => then(self, value),
            None => self,
        }
    }

    /// Whether or not to follow redirects
    fn follow_redirects(self, follow: RedirectPolicy) -> Self;
}

impl HttpRequestExt for http::request::Builder {
    fn follow_redirects(self, follow: RedirectPolicy) -> Self {
        self.extension(follow)
    }
}

pub trait HttpClient: 'static + Send + Sync {
    fn type_name(&self) -> &'static str;

    fn user_agent(&self) -> Option<&HeaderValue>;

    fn send(
        &self,
        req: http::Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>>;

    fn get(
        &self,
        uri: &str,
        body: AsyncBody,
        follow_redirects: bool,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let request = Builder::new()
            .uri(uri)
            .follow_redirects(if follow_redirects {
                RedirectPolicy::FollowAll
            } else {
                RedirectPolicy::NoFollow
            })
            .body(body);

        match request {
            Ok(request) => self.send(request),
            Err(e) => Box::pin(async move { Err(e.into()) }),
        }
    }

    fn post_json(
        &self,
        uri: &str,
        body: AsyncBody,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let request = Builder::new()
            .uri(uri)
            .method(Method::POST)
            .header("Content-Type", "application/json")
            .body(body);

        match request {
            Ok(request) => self.send(request),
            Err(e) => Box::pin(async move { Err(e.into()) }),
        }
    }

    fn proxy(&self) -> Option<&Url>;

    #[cfg(feature = "test-support")]
    fn as_fake(&self) -> &FakeHttpClient {
        panic!("called as_fake on {}", type_name::<Self>())
    }

    fn send_multipart_form<'a>(
        &'a self,
        _url: &str,
        _request: reqwest::multipart::Form,
    ) -> BoxFuture<'a, anyhow::Result<Response<AsyncBody>>> {
        future::ready(Err(anyhow!("not implemented"))).boxed()
    }
}

/// An [`HttpClient`] that may have a proxy.
#[derive(Deref)]
pub struct HttpClientWithProxy {
    #[deref]
    client: Arc<dyn HttpClient>,
    proxy: Option<Url>,
}

impl HttpClientWithProxy {
    /// Returns a new [`HttpClientWithProxy`] with the given proxy URL.
    pub fn new(client: Arc<dyn HttpClient>, proxy_url: Option<String>) -> Self {
        let proxy_url = proxy_url
            .and_then(|proxy| proxy.parse().ok())
            .or_else(read_proxy_from_env);

        Self::new_url(client, proxy_url)
    }
    pub fn new_url(client: Arc<dyn HttpClient>, proxy_url: Option<Url>) -> Self {
        Self {
            client,
            proxy: proxy_url,
        }
    }
}

impl HttpClient for HttpClientWithProxy {
    fn send(
        &self,
        req: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        self.client.send(req)
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        self.client.user_agent()
    }

    fn proxy(&self) -> Option<&Url> {
        self.proxy.as_ref()
    }

    fn type_name(&self) -> &'static str {
        self.client.type_name()
    }

    #[cfg(feature = "test-support")]
    fn as_fake(&self) -> &FakeHttpClient {
        self.client.as_fake()
    }

    fn send_multipart_form<'a>(
        &'a self,
        url: &str,
        form: reqwest::multipart::Form,
    ) -> BoxFuture<'a, anyhow::Result<Response<AsyncBody>>> {
        self.client.send_multipart_form(url, form)
    }
}

/// An [`HttpClient`] that has a base URL.
pub struct HttpClientWithUrl {
    base_url: Mutex<String>,
    client: HttpClientWithProxy,
}

impl std::ops::Deref for HttpClientWithUrl {
    type Target = HttpClientWithProxy;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl HttpClientWithUrl {
    /// Returns a new [`HttpClientWithUrl`] with the given base URL.
    pub fn new(
        client: Arc<dyn HttpClient>,
        base_url: impl Into<String>,
        proxy_url: Option<String>,
    ) -> Self {
        let client = HttpClientWithProxy::new(client, proxy_url);

        Self {
            base_url: Mutex::new(base_url.into()),
            client,
        }
    }

    pub fn new_url(
        client: Arc<dyn HttpClient>,
        base_url: impl Into<String>,
        proxy_url: Option<Url>,
    ) -> Self {
        let client = HttpClientWithProxy::new_url(client, proxy_url);

        Self {
            base_url: Mutex::new(base_url.into()),
            client,
        }
    }

    /// Returns the base URL.
    pub fn base_url(&self) -> String {
        self.base_url.lock().clone()
    }

    /// Sets the base URL.
    pub fn set_base_url(&self, base_url: impl Into<String>) {
        let base_url = base_url.into();
        *self.base_url.lock() = base_url;
    }

    /// Builds a URL using the given path.
    pub fn build_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url(), path)
    }

    /// Builds a Zed API URL using the given path.
    pub fn build_zed_api_url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        let base_url = self.base_url();
        let base_api_url = match base_url.as_ref() {
            "https://zed.dev" => "https://api.zed.dev",
            "https://staging.zed.dev" => "https://api-staging.zed.dev",
            "http://localhost:3000" => "http://localhost:8080",
            other => other,
        };

        Ok(Url::parse_with_params(
            &format!("{}{}", base_api_url, path),
            query,
        )?)
    }

    /// Builds a Zed Cloud URL using the given path.
    pub fn build_zed_cloud_url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        let base_url = self.base_url();
        let base_api_url = match base_url.as_ref() {
            "https://zed.dev" => "https://cloud.zed.dev",
            "https://staging.zed.dev" => "https://cloud.zed.dev",
            "http://localhost:3000" => "http://localhost:8787",
            other => other,
        };

        Ok(Url::parse_with_params(
            &format!("{}{}", base_api_url, path),
            query,
        )?)
    }

    /// Builds a Zed LLM URL using the given path.
    pub fn build_zed_llm_url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        let base_url = self.base_url();
        let base_api_url = match base_url.as_ref() {
            "https://zed.dev" => "https://cloud.zed.dev",
            "https://staging.zed.dev" => "https://llm-staging.zed.dev",
            "http://localhost:3000" => "http://localhost:8787",
            other => other,
        };

        Ok(Url::parse_with_params(
            &format!("{}{}", base_api_url, path),
            query,
        )?)
    }
}

impl HttpClient for HttpClientWithUrl {
    fn send(
        &self,
        req: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        self.client.send(req)
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        self.client.user_agent()
    }

    fn proxy(&self) -> Option<&Url> {
        self.client.proxy.as_ref()
    }

    fn type_name(&self) -> &'static str {
        self.client.type_name()
    }

    #[cfg(feature = "test-support")]
    fn as_fake(&self) -> &FakeHttpClient {
        self.client.as_fake()
    }

    fn send_multipart_form<'a>(
        &'a self,
        url: &str,
        request: reqwest::multipart::Form,
    ) -> BoxFuture<'a, anyhow::Result<Response<AsyncBody>>> {
        self.client.send_multipart_form(url, request)
    }
}

pub fn read_proxy_from_env() -> Option<Url> {
    const ENV_VARS: &[&str] = &[
        "ALL_PROXY",
        "all_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ];

    ENV_VARS
        .iter()
        .find_map(|var| std::env::var(var).ok())
        .and_then(|env| env.parse().ok())
}

pub fn read_no_proxy_from_env() -> Option<String> {
    const ENV_VARS: &[&str] = &["NO_PROXY", "no_proxy"];

    ENV_VARS.iter().find_map(|var| std::env::var(var).ok())
}

#[derive(Default)]
pub struct BlockedHttpClient;

impl BlockedHttpClient {
    pub fn new() -> Self {
        BlockedHttpClient
    }
}

impl HttpClient for BlockedHttpClient {
    fn send(
        &self,
        _req: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        Box::pin(async {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "BlockedHttpClient disallowed request",
            )
            .into())
        })
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn type_name(&self) -> &'static str {
        type_name::<Self>()
    }

    #[cfg(feature = "test-support")]
    fn as_fake(&self) -> &FakeHttpClient {
        panic!("called as_fake on {}", type_name::<Self>())
    }
}

#[cfg(feature = "test-support")]
type FakeHttpHandler = Arc<
    dyn Fn(Request<AsyncBody>) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>>
        + Send
        + Sync
        + 'static,
>;

#[cfg(feature = "test-support")]
pub struct FakeHttpClient {
    handler: Mutex<FakeHttpHandler>,
    user_agent: HeaderValue,
}

#[cfg(feature = "test-support")]
impl FakeHttpClient {
    pub fn create<Fut, F>(handler: F) -> Arc<HttpClientWithUrl>
    where
        Fut: futures::Future<Output = anyhow::Result<Response<AsyncBody>>> + Send + 'static,
        F: Fn(Request<AsyncBody>) -> Fut + Send + Sync + 'static,
    {
        Arc::new(HttpClientWithUrl {
            base_url: Mutex::new("http://test.example".into()),
            client: HttpClientWithProxy {
                client: Arc::new(Self {
                    handler: Mutex::new(Arc::new(move |request| Box::pin(handler(request)))),
                    user_agent: HeaderValue::from_static(type_name::<Self>()),
                }),
                proxy: None,
            },
        })
    }

    pub fn with_404_response() -> Arc<HttpClientWithUrl> {
        Self::create(
            |_| async move { Ok(Response::builder().status(404).body(AsyncBody::empty())?) },
        )
    }

    pub fn with_200_response() -> Arc<HttpClientWithUrl> {
        Self::create(
            |_| async move { Ok(Response::builder().status(200).body(AsyncBody::empty())?) },
        )
    }

    pub fn replace_handler<Fut, F>(&self, new_handler: F)
    where
        Fut: futures::Future<Output = anyhow::Result<Response<AsyncBody>>> + Send + 'static,
        F: Fn(FakeHttpHandler, Request<AsyncBody>) -> Fut + Send + Sync + 'static,
    {
        let mut handler = self.handler.lock();
        let old_handler = handler.clone();
        *handler = Arc::new(move |request| Box::pin(new_handler(old_handler.clone(), request)));
    }
}

#[cfg(feature = "test-support")]
impl fmt::Debug for FakeHttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeHttpClient").finish()
    }
}

#[cfg(feature = "test-support")]
impl HttpClient for FakeHttpClient {
    fn send(
        &self,
        req: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let handler = self.handler.lock().clone();
        handler(req)
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        Some(&self.user_agent)
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn type_name(&self) -> &'static str {
        type_name::<Self>()
    }

    fn as_fake(&self) -> &FakeHttpClient {
        self
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use futures::{AsyncReadExt, executor::block_on};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn fake_client_forwards_request_metadata_and_streamed_response() -> Result<()> {
        block_on(async {
            let client =
                FakeHttpClient::create(|mut request| async move {
                    assert_eq!(request.method(), Method::POST);
                    assert_eq!(request.uri(), "https://example.test/api");
                    assert_eq!(
                        request.headers().get("Content-Type"),
                        Some(&HeaderValue::from_static("application/json"))
                    );
                    let mut body = String::new();
                    request.body_mut().read_to_string(&mut body).await?;
                    assert_eq!(body, "{\"answer\":42}");
                    Ok(Response::builder().status(StatusCode::CREATED).body(
                        AsyncBody::from_reader(futures::io::Cursor::new(b"streamed".to_vec())),
                    )?)
                });
            let mut response = client
                .post_json("https://example.test/api", "{\"answer\":42}".into())
                .await?;
            assert_eq!(response.status(), StatusCode::CREATED);
            let mut payload = String::new();
            response.body_mut().read_to_string(&mut payload).await?;
            assert_eq!(payload, "streamed");
            Ok(())
        })
    }

    #[test]
    fn replacement_handler_can_delegate_without_holding_its_lock() -> Result<()> {
        block_on(async {
            let client = FakeHttpClient::with_200_response();
            client
                .as_fake()
                .replace_handler(|previous, request| async move {
                    let mut response = previous(request).await?;
                    *response.status_mut() = StatusCode::ACCEPTED;
                    Ok(response)
                });
            let response = client
                .get("https://example.test/", AsyncBody::empty(), true)
                .await?;
            assert_eq!(response.status(), StatusCode::ACCEPTED);
            Ok(())
        })
    }

    #[test]
    fn proxy_wrapper_forwards_the_stable_multipart_type() -> Result<()> {
        struct MultipartClient(Arc<AtomicUsize>);
        impl HttpClient for MultipartClient {
            fn type_name(&self) -> &'static str {
                "MultipartClient"
            }
            fn user_agent(&self) -> Option<&HeaderValue> {
                None
            }
            fn proxy(&self) -> Option<&Url> {
                None
            }
            fn send(
                &self,
                _: Request<AsyncBody>,
            ) -> BoxFuture<'static, Result<Response<AsyncBody>>> {
                Box::pin(async { Err(anyhow!("unexpected ordinary request")) })
            }
            fn send_multipart_form<'a>(
                &'a self,
                url: &str,
                _: reqwest::multipart::Form,
            ) -> BoxFuture<'a, Result<Response<AsyncBody>>> {
                assert_eq!(url, "https://example.test/upload");
                self.0.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok(Response::new(AsyncBody::empty())) })
            }
        }
        block_on(async {
            let calls = Arc::new(AtomicUsize::new(0));
            let proxy = Url::parse("http://proxy.example.test:8080")?;
            let client = HttpClientWithProxy::new_url(
                Arc::new(MultipartClient(calls.clone())),
                Some(proxy.clone()),
            );
            assert_eq!(client.proxy(), Some(&proxy));
            client
                .send_multipart_form(
                    "https://example.test/upload",
                    reqwest::multipart::Form::new().text("key", "value"),
                )
                .await?;
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            Ok(())
        })
    }

    #[test]
    fn dropping_a_pending_request_releases_its_stream() -> Result<()> {
        struct Reader(Arc<AtomicUsize>);
        impl futures::AsyncRead for Reader {
            fn poll_read(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
                _: &mut [u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                std::task::Poll::Pending
            }
        }
        impl Drop for Reader {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let drops = Arc::new(AtomicUsize::new(0));
        let client = FakeHttpClient::create(|request| async move {
            futures::future::pending::<()>().await;
            drop(request);
            Ok(Response::new(AsyncBody::empty()))
        });
        let request = Request::builder()
            .uri("https://example.test/pending")
            .body(AsyncBody::from_reader(Reader(drops.clone())))?;
        let future = client.send(request);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(future);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[test]
    fn blocked_client_reports_permission_denied() {
        block_on(async {
            let client = BlockedHttpClient::new();
            let result = client
                .get("https://example.test/", AsyncBody::empty(), false)
                .await;
            let error = match result {
                Ok(_) => panic!("blocked client unexpectedly permitted a request"),
                Err(error) => error,
            };
            assert_eq!(
                error
                    .downcast_ref::<std::io::Error>()
                    .map(std::io::Error::kind),
                Some(std::io::ErrorKind::PermissionDenied)
            );
        });
    }
}
