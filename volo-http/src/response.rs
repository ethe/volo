use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{ready, stream};
use http_body_util::{Full, StreamBody};
use hyper::body::{Body, Bytes, Frame};
use pin_project_lite::pin_project;

use crate::DynError;

pin_project! {
    #[project = RespBodyProj]
    pub enum RespBody {
        Stream {
            #[pin] inner: StreamBody<stream::Iter<Box<dyn Iterator<Item = Result<Frame<Bytes>, DynError>> + Send + Sync>>>,
        },
        Full {
            #[pin] inner: Full<Bytes>,
        },
    }
}

impl Body for RespBody {
    type Data = Bytes;

    type Error = DynError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.project() {
            RespBodyProj::Stream { inner } => inner.poll_frame(cx),
            RespBodyProj::Full { inner } => {
                Poll::Ready(ready!(inner.poll_frame(cx)).map(|result| Ok(result.unwrap())))
            }
        }
    }
}

impl From<Full<Bytes>> for RespBody {
    fn from(value: Full<Bytes>) -> Self {
        Self::Full { inner: value }
    }
}

impl From<String> for RespBody {
    fn from(value: String) -> Self {
        Self::Full {
            inner: Full::new(value.into()),
        }
    }
}

impl From<&'static str> for RespBody {
    fn from(value: &'static str) -> Self {
        Self::Full {
            inner: Full::new(value.into()),
        }
    }
}
