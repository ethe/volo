use std::future::Future;

use http::{Response, StatusCode};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};

use crate::{response::RespBody, DynError, HttpContext, HttpContextInner};

pub type DynService = motore::BoxCloneService<HttpContext, Incoming, Response<RespBody>, DynError>;

#[derive(Clone)]
pub struct Router {
    inner: matchit::Router<DynService>,
}

impl Router {
    pub fn build() -> RouterBuilder {
        Default::default()
    }
}

impl motore::Service<(), (HttpContextInner, Incoming)> for Router {
    type Response = Response<RespBody>;

    type Error = DynError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + Send + 'cx
    where
        HttpContextInner: 'cx,
        Self: 'cx;

    fn call<'cx, 's>(
        &'s self,
        _cx: &'cx mut (),
        cxreq: (HttpContextInner, Incoming),
    ) -> Self::Future<'cx>
    where
        's: 'cx,
    {
        async move {
            let (cx, req) = cxreq;
            if let Ok(matched) = self.inner.at(cx.uri.path()) {
                let mut context = HttpContext {
                    peer: cx.peer,
                    method: cx.method,
                    uri: cx.uri.clone(),
                    version: cx.version,
                    headers: cx.headers,
                    extensions: cx.extensions,
                    params: unsafe { std::mem::transmute(matched.params) },
                };
                matched.value.call(&mut context, req).await
            } else {
                Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Full::new(Bytes::new()).into())
                    .unwrap())
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct DispatchService<S> {
    inner: S,
}

impl<S> DispatchService<S> {
    fn new(service: S) -> Self {
        Self { inner: service }
    }
}

impl<S, RB> motore::Service<HttpContext, Incoming> for DispatchService<S>
where
    S: motore::Service<HttpContext, Incoming, Response = Response<RB>> + Send + Sync + 'static,
    S::Error: std::error::Error + Send + Sync,
    RB: Into<RespBody>,
{
    type Response = Response<RespBody>;

    type Error = DynError;

    type Future<'cx> = impl Future<Output = Result<Self::Response, Self::Error>> + Send + 'cx
    where
        HttpContext: 'cx,
        Self: 'cx;

    fn call<'cx, 's>(&'s self, cx: &'cx mut HttpContext, req: Incoming) -> Self::Future<'cx>
    where
        's: 'cx,
    {
        async move {
            self.inner
                .call(cx, req)
                .await
                .map(|resp| {
                    let (parts, body) = resp.into_parts();
                    Response::from_parts(parts, body.into())
                })
                .map_err(|e| Box::new(e) as DynError)
        }
    }
}

#[derive(Default)]
pub struct RouterBuilder {
    routes: matchit::Router<DynService>,
}

impl RouterBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn route<R, S, RB>(mut self, route: R, handler: S) -> Self
    where
        R: Into<String>,
        S: motore::Service<HttpContext, Incoming, Response = Response<RB>>
            + Send
            + Sync
            + Clone
            + 'static,
        S::Error: std::error::Error + Send + Sync,
        RB: Into<RespBody>,
    {
        if let Err(e) = self.routes.insert(
            route,
            motore::BoxCloneService::new(DispatchService::new(handler)),
        ) {
            panic!("routing error: {e}");
        }
        self
    }

    pub fn build(self) -> Router {
        Router { inner: self.routes }
    }
}
