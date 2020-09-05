use std::io::{Read, Write};
use std::sync::{Arc};
use std::net::TcpStream;
use anyhow::{anyhow, Result};

mod protocol;
mod parser;
mod tofu;

use crate::parser::{parse_response_header, parse_text_gemini};
use crate::protocol::{Document, ResponseHeader, ResponseStatus, Line};
use crate::tofu::GeminiCertificateVerifier;

fn fetch(target: &str, config: Arc<rustls::ClientConfig>)
    -> Result<(ResponseHeader, Option<Document>)>
{
    fetch_(target, config, 0)
}

fn fetch_(target: &str, config: Arc<rustls::ClientConfig>, depth: u8)
    -> Result<(ResponseHeader, Option<Document>)>
{
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
        RedirectTemporary | RedirectPermanent =>
            fetch_(&header.meta, config, depth + 1),

        // Only read the response body if we got a Success response status
        Success =>
            if header.meta.starts_with("text/gemini") {
                let body = std::str::from_utf8(body)?;
                let (_, text) = parse_text_gemini(body).map_err(
                    |e| anyhow!("text/gemini parsing failed: {}", e))?;
                Ok((header, Some(text)))
            } else if header.meta.starts_with("text/") {
                // Read other text/ MIME types as a single plain-text line
                let body = std::str::from_utf8(body)?;
                Ok((header, Some(vec![Line::Text(body.to_string())])))
            } else {
                Err(anyhow!("Unknown meta: {}", header.meta))
            },

        // Otherwise, return the header
        _ => Ok((header, None)),
    }
}

fn main() -> Result<()> {
    let dirs = directories::ProjectDirs::from("com", "mkeeter", "titan")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other,
                                           "Could not get ProjectDirs"))?;
    let db = sled::open(dirs.data_dir())?;

    let mut config = rustls::ClientConfig::new();
    let verifier = GeminiCertificateVerifier::new(&db)?;
    config.dangerous().set_certificate_verifier(Arc::new(verifier));
    let config = Arc::new(config);

    fetch("gemini://gemini.circumlunar.space", config)?;
    Ok(())
}
