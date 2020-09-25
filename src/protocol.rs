use std::convert::TryFrom;
use anyhow::{Error, anyhow};

#[derive(Debug, Eq, PartialEq)]
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

pub type ResponseHeader<'a> = (ResponseStatus, &'a str);

#[derive(Debug)]
pub struct Response<'a> {
    pub status: ResponseStatus,
    pub meta: &'a str,
    pub body: &'a [u8],
}

#[derive(Debug, Eq, PartialEq)]
pub enum Line_<'a, T> {
    Text(T),
    BareLink(&'a str),
    NamedLink { url: &'a str, name: T },
    Pre { alt: Option<&'a str>, text: T },
    H1(T),
    H2(T),
    H3(T),
    List(T),
    Quote(T),
}

pub type Line<'a> = Line_<'a, &'a str>;
