use std::sync::Arc;

pub use pilota::thrift::Message;
use pilota::thrift::{
    DecodeError, EncodeError, TAsyncInputProtocol, TInputProtocol, TLengthProtocol,
    TMessageIdentifier, TOutputProtocol,
};

#[async_trait::async_trait]
pub trait EntryMessage: Sized + Send {
    fn encode<T: TOutputProtocol>(&self, protocol: &mut T) -> Result<(), EncodeError>;

    fn decode<T: TInputProtocol>(
        protocol: &mut T,
        msg_ident: &TMessageIdentifier,
    ) -> Result<Self, DecodeError>;

    async fn decode_async<T: TAsyncInputProtocol>(
        protocol: &mut T,
        msg_ident: &TMessageIdentifier,
    ) -> Result<Self, DecodeError>;

    fn size<T: TLengthProtocol>(&self, protocol: &mut T) -> usize;
}

#[async_trait::async_trait]
impl<Message> EntryMessage for Arc<Message>
where
    Message: EntryMessage + Sync,
{
    #[inline]
    fn encode<T: TOutputProtocol>(&self, protocol: &mut T) -> Result<(), EncodeError> {
        (**self).encode(protocol)
    }

    #[inline]
    fn decode<T: TInputProtocol>(
        protocol: &mut T,
        msg_ident: &TMessageIdentifier,
    ) -> Result<Self, DecodeError> {
        Message::decode(protocol, msg_ident).map(Arc::new)
    }

    #[inline]
    async fn decode_async<T: TAsyncInputProtocol>(
        protocol: &mut T,
        msg_ident: &TMessageIdentifier,
    ) -> Result<Self, DecodeError> {
        Message::decode_async(protocol, msg_ident)
            .await
            .map(Arc::new)
    }

    #[inline]
    fn size<T: TLengthProtocol>(&self, protocol: &mut T) -> usize {
        (**self).size(protocol)
    }
}
