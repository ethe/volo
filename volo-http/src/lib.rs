#![feature(impl_trait_in_assoc_type)]

pub mod layer;
pub mod response;
pub mod route;

use std::{future::Future, net::SocketAddr};

use http::{Extensions, HeaderMap, HeaderValue, Method, Uri, Version};
use hyper::{
    body::{Body, Incoming},
    server::conn::http1,
    Request, Response,
};
use hyper_util::rt::TokioIo;
use matchit::Params;
use tokio::net::TcpListener;

pub type DynError = Box<dyn std::error::Error + Send + Sync>;

pub struct HttpContextInner {
    pub(crate) peer: SocketAddr,

    pub(crate) method: Method,
    pub(crate) uri: Uri,
    pub(crate) version: Version,
    pub(crate) headers: HeaderMap<HeaderValue>,
    pub(crate) extensions: Extensions,
}

pub struct HttpContext {
    pub peer: SocketAddr,
    pub method: Method,
    pub uri: Uri,
    pub version: Version,
    pub headers: HeaderMap<HeaderValue>,
    pub extensions: Extensions,

    pub params: Params<'static, 'static>,
}

#[derive(Clone)]
pub struct MotoreService<S> {
    peer: SocketAddr,
    inner: S,
}

impl<RespBody, S> hyper::service::Service<Request<Incoming>> for MotoreService<S>
where
    RespBody: Body<Error = DynError>,
    // <RespBody as Body>::Error: std::error::Error + Sync + Send,
    S: motore::Service<(), (HttpContextInner, Incoming), Response = Response<RespBody>> + Clone,
    S::Error: Into<DynError>,
{
    type Response = S::Response;

    type Error = S::Error;

    type Future = impl Future<Output = Result<Self::Response, Self::Error>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let s = self.inner.clone();
        let peer = self.peer;
        async move {
            let (parts, req) = req.into_parts();
            let cx = HttpContextInner {
                peer,
                method: parts.method,
                uri: parts.uri,
                version: parts.version,
                headers: parts.headers,
                extensions: parts.extensions,
            };
            s.call(&mut (), (cx, req)).await
        }
    }
}

pub async fn serve<RespBody, S>(s: S) -> Result<(), DynError>
where
    RespBody: 'static + Body<Error = DynError> + Send,
    // <RespBody as Body>::Error: std::error::Error + Send + Sync,
    <RespBody as Body>::Data: Send,
    S: 'static
        + motore::Service<(), (HttpContextInner, Incoming), Response = Response<RespBody>>
        + Send
        + Sync
        + Clone,
    S::Response: Body,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
{
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let listener = TcpListener::bind(addr).await?;

    loop {
        let s = s.clone();
        let (stream, peer) = listener.accept().await?;

        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, MotoreService { peer, inner: s })
                .await
            {
                tracing::error!("error serving connection: {:?}", err);
            }
        });
    }
}

#[cfg(test)]
mod tests {

    use std::convert::Infallible;

    use http::Response;
    use http_body_util::Full;
    use hyper::body::{Bytes, Incoming};
    use motore::service::service_fn;

    use crate::{route::Router, serve, HttpContext};

    async fn hello(
        _cx: &mut HttpContext,
        _request: Incoming,
    ) -> Result<Response<Full<Bytes>>, Infallible> {
        Ok(Response::new("hello, world".into()))
    }

    #[tokio::test]
    async fn routing() {
        let router = Router::build().route("/", service_fn(hello)).build();
        serve(router).await.unwrap();
    }
}
