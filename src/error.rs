use std::fmt;

pub trait ResultExt: Sized {
    type Element;
    type Error: fmt::Debug;

    fn myself(self) -> Result<Self::Element, Self::Error>;
    fn unwrap_or_exit(self) -> Self::Element {
        match self.myself() {
            Ok(x) => x,
            Err(e) => {
                eprintln!("ERROR: {:?}", e);
                std::process::exit(1);
            }
        }
    }
}

impl<T, E: fmt::Debug> ResultExt for Result<T, E> {
    type Element = T;
    type Error = E;

    fn myself(self) -> Result<T, E> {
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0:?}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("unexceptable input")]
    UnexceptableInput,

    #[error("no audio")]
    NoAudio,

    #[error("no video")]
    NoVideo,

    #[error("url error: {0:?}")]
    Url(#[from] url::ParseError),

    #[error("io error: {0:?}")]
    Io(#[from] std::io::Error),

    #[error("network error")]
    Network,

    #[error("no file: {0:?}")]
    NoFile(#[from] which::Error),

    #[error("no master.json")]
    NoMasterJson,

    #[error("base64 decode error: {0:?}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("tokio task join error: {0:?}")]
    TaskJoin(#[from] tokio::task::JoinError),
}