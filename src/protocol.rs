use std::convert::TryFrom;
use anyhow::{Error, anyhow};

#[derive(Debug)]
pub enum ResponseStatus {
    Input,
    SensitiveInput,
    Success,
    RedirectTemporary,
    RedirectPermanent,
    TemporaryFailure,
    ServerUnavailable,
    CGIError,
    ProxyError,
    SlowDown,
    PermanentFailure,
    NotFound,
    Gone,
    ProxyRequestRefused,
    BadRequest,
    ClientCertificateRequired,
    CertificateNotAuthorized,
    CertificateNotValid,
}

impl TryFrom<u32> for ResponseStatus {
    type Error = Error;
    fn try_from(v: u32) -> Result<Self, Self::Error> {
        use ResponseStatus::*;
        Ok(match v {
            10 => Input,
            11 => SensitiveInput,
            20 => Success,
            30 => RedirectTemporary,
            31 => RedirectPermanent,
            40 => TemporaryFailure,
            41 => ServerUnavailable,
            42 => CGIError,
            43 => ProxyError,
            44 => SlowDown,
            50 => PermanentFailure,
            51 => NotFound,
            52 => Gone,
            53 => ProxyRequestRefused,
            59 => BadRequest,
            60 => ClientCertificateRequired,
            61 => CertificateNotAuthorized,
            62 => CertificateNotValid,
            _ => return Err(anyhow!("Invalid status code {}", v)),
        })
    }
}

#[derive(Debug)]
pub struct ResponseHeader {
    pub status: ResponseStatus,
    pub meta: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Line {
    Text(String),
    Link { url: String, name: Option<String> },
    Pre { alt: Option<String>, text: String },
    H1(String),
    H2(String),
    H3(String),
    List(String),
    Quote(String),
}
