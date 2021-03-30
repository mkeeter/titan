use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid status code `{0}`")]
    InvalidStatusCode(u32),

    #[error("parsing failed")]
    ParseError,

    #[error("too many redirects")]
    TooManyRedirects,

    #[error("failed to write to db `{0}`")]
    DBWriteError(String),

    #[error("invalid URL scheme `{0}`")]
    InvalidURLScheme(String),

    #[error("no hostname in `{0}`")]
    NoHostname(String),

    #[error("unknown metatype `{0}`")]
    UnknownMeta(String),

    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),

    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error(transparent)]
    SledError(#[from] sled::Error),

    #[error(transparent)]
    TLSError(#[from] rustls::TLSError),

    #[error(transparent)]
    InvalidDNSNameError(#[from] webpki::InvalidDNSNameError),

    #[error(transparent)]
    IOError(#[from] std::io::Error),
}
