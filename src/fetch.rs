use std::io::{Read, Write};
use std::sync::{Arc};
use std::net::TcpStream;

use anyhow::{anyhow, Result};

use crate::document::Document;
use crate::parser::{parse_response_header, parse_text_gemini};
use crate::protocol::{Line, ResponseHeader, ResponseStatus};

pub trait Fetch {
    fn input(&mut self, prompt: &str, is_sensitive: bool) -> Result<String>;
    fn display(&mut self, doc: &Document) -> Result<()>;
    fn header(&mut self, header: &ResponseHeader) -> Result<()>;
}

pub fn fetch<F: Fetch>(target: &str, config: Arc<rustls::ClientConfig>,
                       cb: &mut F) -> Result<()> {
    fetch_(target, config, cb, 0)
}

fn fetch_<F: Fetch>(target: &str, config: Arc<rustls::ClientConfig>,
                    cb: &mut F, depth: u8) -> Result<()> {
    println!("Fetching {}", target);
    if depth >= 5 {
        return Err(anyhow!("Too much recursion"));
    }

    let url = url::Url::parse(target)?;
    if url.scheme() != "gemini" {
        return Err(anyhow!("Invalid URL scheme: {}", url.scheme()));
    }

    let hostname = url.host_str()
        .ok_or_else(|| anyhow!("Error: no hostname in {}", target))?;
    let dns_name = webpki::DNSNameRef::try_from_ascii_str(hostname)?;
    let mut sess = rustls::ClientSession::new(&config, dns_name);

    let port = url.port().unwrap_or(1965);
    let mut sock = TcpStream::connect(format!("{}:{}", hostname, port))?;
    let mut tls = rustls::Stream::new(&mut sess, &mut sock);

    tls.write_all(format!("{}\r\n", target).as_bytes())?;

    let mut plaintext = Vec::new();
    let rc = tls.read_to_end(&mut plaintext);

    // The server should cleanly close the connection at the end of the
    // message, which returns an error from read_to_end but is actually okay.
    if let Err(err) = rc {
        if err.kind() != std::io::ErrorKind::ConnectionAborted {
            return Err(err.into());
        }
    }

    let (body, header) = parse_response_header(&plaintext).map_err(
        |e| anyhow!("Header parsing failed: {}", e))?;

    use ResponseStatus::*;
    match header.status {
        RedirectTemporary | RedirectPermanent => {
            return fetch_(&header.meta, config, cb, depth + 1);
        },

        Input | SensitiveInput => {
            let input = cb.input(&header.meta, header.status == SensitiveInput)?;
            let has_query = url.query().is_some();

            // Recast the URL variable to be mutable in this block
            let mut url = url;
            {   // Modify the URL to include the query string
                let mut segs = url.path_segments_mut()
                    .map_err(|_| anyhow!("Could not get path segments"))?;
                if has_query {
                    segs.pop();
                }
                segs.extend(&["?", &input]);
            }
            return fetch_(url.as_str(), config, cb, depth + 1);
        },
        // Only read the response body if we got a Success response status
        Success =>
            if header.meta.starts_with("text/gemini") {
                let body = std::str::from_utf8(body)?;
                let (_, doc) = parse_text_gemini(body).map_err(
                    |e| anyhow!("text/gemini parsing failed: {}", e))?;
                cb.display(&doc)?;
            } else if header.meta.starts_with("text/") {
                // Read other text/ MIME types as a single preformatted line
                let body = std::str::from_utf8(body)?;
                let text = Line::Pre { alt: None, text: body.split('\n').collect() };
                cb.display(&Document(vec![text]))?;
            } else {
                return Err(anyhow!("Unknown meta: {}", header.meta));
            },

        // Otherwise, invoke the header cb
        _ => { cb.header(&header)?; }
    }
    Ok(())
}
