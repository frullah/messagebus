use core::fmt;
use std::{any::type_name, borrow::Cow};

use thiserror::Error;
use tokio::sync::oneshot;

use crate::{
    envelop::{IntoBoxedMessage, TypeTag, TypeTagged},
    Message,
};

pub trait StdSyncSendError: std::error::Error + TypeTagged + Send + Sync + Unpin + 'static {}
impl<T: std::error::Error + TypeTagged + Send + Sync + Unpin + 'static> StdSyncSendError for T {}

#[derive(Debug, Error)]
pub enum VoidError {}

impl TypeTagged for VoidError {
    fn type_name(&self) -> Cow<str> {
        type_name::<VoidError>().into()
    }

    fn type_tag(&self) -> TypeTag {
        type_name::<VoidError>().into()
    }

    fn type_tag_() -> TypeTag
    where
        Self: Sized,
    {
        type_name::<VoidError>().into()
    }
}

#[derive(Debug, Error)]
pub enum SendError<M: fmt::Debug> {
    #[error("Closed")]
    Closed(M),

    #[error("Full")]
    Full(M),
}

impl<M: Message> SendError<M> {
    pub fn into_boxed(self) -> SendError<Box<dyn Message>> {
        match self {
            SendError::Closed(m) => SendError::Closed(m.into_boxed()),
            SendError::Full(m) => SendError::Closed(m.into_boxed()),
        }
    }
}

#[derive(Debug, Error)]
pub enum Error<M: fmt::Debug + 'static = (), E: StdSyncSendError = VoidError> {
    #[error("Message Send Error: {0}")]
    SendError(#[from] SendError<M>),

    #[error("NoResponse")]
    NoResponse,

    #[error("NoReceivers")]
    NoReceivers,

    #[error("AddListenerError")]
    AddListenerError,

    #[error("MessageCastError")]
    MessageCastError,

    #[error("Other({0})")]
    Other(E),

    #[error("Serialization({0})")]
    Serialization(#[from] erased_serde::Error),

    #[error("Other({0})")]
    OtherBoxed(Box<dyn StdSyncSendError>),

    #[error("WrongMessageType()")]
    WrongMessageType(M),
}

impl<M: Message, E: StdSyncSendError> Error<M, E> {
    pub fn into_dyn(self) -> Error<M> {
        match self {
            Error::SendError(inner) => Error::SendError(inner),
            Error::NoResponse => Error::NoReceivers,
            Error::NoReceivers => Error::NoReceivers,
            Error::Serialization(s) => Error::Serialization(s),
            Error::Other(inner) => Error::OtherBoxed(Box::new(inner) as _),
            Error::OtherBoxed(inner) => Error::OtherBoxed(inner),
            Error::WrongMessageType(inner) => Error::WrongMessageType(inner),
            Error::AddListenerError => Error::AddListenerError,
            Error::MessageCastError => Error::MessageCastError,
        }
    }

    pub fn map<U: From<Box<dyn StdSyncSendError>> + StdSyncSendError>(self) -> Error<M, U> {
        match self {
            Error::SendError(inner) => Error::SendError(inner),
            Error::NoResponse => Error::NoReceivers,
            Error::NoReceivers => Error::NoReceivers,
            Error::Serialization(s) => Error::Serialization(s),
            Error::Other(_) => panic!("expected boxed error!"),
            Error::OtherBoxed(inner) => Error::Other(inner.into()),
            Error::WrongMessageType(inner) => Error::WrongMessageType(inner),
            Error::AddListenerError => Error::AddListenerError,
            Error::MessageCastError => Error::MessageCastError,
        }
    }
}

impl<E: StdSyncSendError> Error<(), E> {
    pub fn specify<M: fmt::Debug>(self) -> Error<M, E> {
        match self {
            Error::SendError(_) => panic!("cannot specify type on typed error"),
            Error::WrongMessageType(_) => panic!("cannot specify type on typed error"),
            Error::NoResponse => Error::NoReceivers,
            Error::NoReceivers => Error::NoReceivers,
            Error::Serialization(s) => Error::Serialization(s),
            Error::Other(inner) => Error::Other(inner),
            Error::OtherBoxed(inner) => Error::OtherBoxed(inner),
            Error::AddListenerError => Error::AddListenerError,
            Error::MessageCastError => Error::MessageCastError,
        }
    }
}

impl<M: fmt::Debug, E: StdSyncSendError> From<oneshot::error::RecvError> for Error<M, E> {
    fn from(_: oneshot::error::RecvError) -> Self {
        Error::NoResponse
    }
}

impl Error<Box<dyn Message>> {
    pub fn from_typed<M: Message>(err: Error<M>) -> Self {
        match err {
            Error::SendError(SendError::Closed(m)) => {
                Error::SendError(SendError::Closed(m.into_boxed()))
            }
            Error::SendError(SendError::Full(m)) => {
                Error::SendError(SendError::Full(m.into_boxed()))
            }
            Error::WrongMessageType(m) => Error::WrongMessageType(m.into_boxed()),
            Error::NoResponse => Error::NoReceivers,
            Error::NoReceivers => Error::NoReceivers,
            Error::Serialization(s) => Error::Serialization(s),
            Error::Other(inner) => Error::Other(inner),
            Error::OtherBoxed(inner) => Error::OtherBoxed(inner),
            Error::AddListenerError => Error::AddListenerError,
            Error::MessageCastError => Error::MessageCastError,
        }
    }
}
