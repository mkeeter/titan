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

#[derive(Debug)]
pub struct ResponseHeader<'a> {
    pub status: ResponseStatus,
    pub meta: &'a str,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Line_<'a, T> {
    Text(T),

    // TODO: switch to BareLink and NamedLink, as they're rendered differently
    Link { url: &'a str, name: Option<T> },
    Pre { alt: Option<&'a str>, text: Vec<&'a str> },
    H1(T),
    H2(T),
    H3(T),
    List(T),
    Quote(T),
}

impl<'a, T> Line_<'a, Vec<T>> {
    pub fn len(&self) -> usize {
        use Line_::*;
        match self {
            Text(t) | H1(t) | H2(t) | H3(t) | List(t) | Quote(t) => t.len(),
            Link { name: Some(name), .. } => name.len(),
            Link { name: None, .. } => 1,
            Pre { text, ..} => text.len(),
        }
    }
}

pub type Line<'a> = Line_<'a, &'a str>;
