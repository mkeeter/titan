use std::io::{Read, Write};
use std::sync::{Arc};
use std::net::TcpStream;
use anyhow::{anyhow, Result};

mod protocol;
mod parser;
mod tofu;

use crate::parser::{parse_response_header, parse_text_gemini};
use crate::protocol::{ResponseHeader, ResponseStatus, Line};
use crate::tofu::GeminiCertificateVerifier;

fn talk(hostname: &str, page: &str, config: Arc<rustls::ClientConfig>)
    -> Result<(ResponseHeader, Option<Vec<Line>>)>
{
    let dns_name = webpki::DNSNameRef::try_from_ascii_str(hostname)?;
    let mut sess = rustls::ClientSession::new(&config, dns_name);

    let mut sock = TcpStream::connect(format!("{}:1965", hostname))?;
    let mut tls = rustls::Stream::new(&mut sess, &mut sock);
    tls.write_all(format!("gemini://{}/{}\r\n", hostname, page).as_bytes())?;

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
    if header.status != ResponseStatus::Success {
        return Ok((header, None));
    }
    if header.meta.starts_with("text/gemini") {
        let body = std::str::from_utf8(body)?;
        let (_, text) = parse_text_gemini(body).map_err(
            |e| anyhow!("text/gemini parsing failed: {}", e))?;
        return Ok((header, Some(text)));
    }
    Err(anyhow!("Unknown meta {}", header.meta))
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

    talk("gemini.circumlunar.space", "docs/specification.gmi", config)?;
    Ok(())
}
