use std::io::{Read, Write};
use std::sync::{Arc};
use std::net::TcpStream;

use crate::Error;
use crate::parser::{parse_response, parse_text_gemini};
use crate::protocol::{Line, ResponseStatus, Response};
use crate::document::Document;

pub fn read(config: &Arc<rustls::ClientConfig>, url: &url::Url)
    -> Result<Vec<u8>, Error>
{
    if url.scheme() != "gemini" {
        return Err(Error::InvalidURLScheme(url.scheme().to_owned()));
    }
    let hostname = url.host_str()
        .ok_or_else(|| Error::NoHostname(url.as_str().to_owned()))?;
    let dns_name = webpki::DNSNameRef::try_from_ascii_str(hostname)?;
    let mut sess = rustls::ClientSession::new(config, dns_name);

    let port = url.port().unwrap_or(1965);
    let mut sock = TcpStream::connect(format!("{}:{}", hostname, port))?;
    let mut tls = rustls::Stream::new(&mut sess, &mut sock);

    tls.write_all(format!("{}\r\n", url.as_str()).as_bytes())?;

    let mut plaintext = Vec::new();
    let rc = tls.read_to_end(&mut plaintext);

    // The server should cleanly close the connection at the end of the
    // message, which returns an error from read_to_end but is actually okay.
    if let Err(err) = rc {
        if err.kind() != std::io::ErrorKind::ConnectionAborted {
            return Err(err.into());
        }
    }
    Ok(plaintext)
}

////////////////////////////////////////////////////////////////////////////////
// Experimental zone!

use ouroboros::self_referencing;
#[self_referencing]
struct OwnedResponse {
    data: Vec<u8>,

    #[borrows(data)]
    #[covariant]
    response: Response<'this>
}

use std::ops::Deref;
impl Deref for OwnedResponse {
    type Target = [u8];
    fn deref(&self) -> &Self::Target { self.borrow_response().body }
}
unsafe impl stable_deref_trait::StableDeref for OwnedResponse {} // marker

impl OwnedResponse {
    fn status(&self) -> ResponseStatus {
        self.borrow_response().status
    }
    fn meta(&self) -> &str {
        self.borrow_response().meta
    }
}

#[self_referencing]
pub struct OwnedDocument {
    data: OwnedResponse,

    #[borrows(data)]
    #[covariant]
    doc: Option<Document<'this>>
}

////////////////////////////////////////////////////////////////////////////////

pub fn fetch(config: &Arc<rustls::ClientConfig>, url: url::Url)
    -> Result<OwnedDocument, Error>
{
    fetch_(config, url, 0)
}

fn fetch_(config: &Arc<rustls::ClientConfig>, url: url::Url, depth: u8)
    -> Result<OwnedDocument, Error>
{
    if depth >= 5 {
        return Err(Error::TooManyRedirects);
    }

    let plaintext = read(config, &url)?;
    let response = OwnedResponse::try_new(plaintext, |p| parse_response(p))?;

    if response.status() == ResponseStatus::Success {
        if response.meta().starts_with("text/gemini") {
            OwnedDocument::try_new(response,
                |body| {
                    let body = std::str::from_utf8(body)?;
                    let (_, doc) = parse_text_gemini(body)
                        .map_err(|_| Error::ParseError)?;
                    Ok(Some(doc))
                })
        } else if response.meta().starts_with("text/") {
            OwnedDocument::try_new(response,
                |body| {
                    // Read other text/ MIME types as a single preformatted line
                    let body = std::str::from_utf8(body)?;
                    let text = Line::Pre { alt: None, text: body };
                    Ok(Some(Document(vec![text])))
                })
        } else {
            return Err(Error::UnknownMeta(response.meta().to_owned()));
        }
    } else {
        Ok(OwnedDocument::new(response, |_| None))
    }
}
