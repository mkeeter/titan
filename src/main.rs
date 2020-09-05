use std::io::{BufRead, Read, Write};
use std::sync::{Arc};
use std::net::TcpStream;
use anyhow::{anyhow, Result};

mod document;
mod protocol;
mod parser;
mod tofu;

use crate::document::Document;
use crate::parser::{parse_response_header, parse_text_gemini};
use crate::protocol::{ResponseHeader, ResponseStatus, Line};
use crate::tofu::GeminiCertificateVerifier;

fn fetch<R, D, H>(target: &str, config: Arc<rustls::ClientConfig>,
                  reader_cb: R, doc_cb: D, header_cb: H)
        -> Result<()>
    where R: FnMut(&str, bool) -> Result<String>,
          D: FnMut(&Document) -> Result<()>,
          H: FnMut(&ResponseHeader) -> Result<()>
{
    fetch_(target, config, reader_cb, doc_cb, header_cb, 0)
}

fn fetch_<R, D, H>(target: &str, config: Arc<rustls::ClientConfig>,
                   mut reader_cb: R, mut doc_cb: D, mut header_cb: H,
                   depth: u8)
        -> Result<()>
    where R: FnMut(&str, bool) -> Result<String>,
          D: FnMut(&Document) -> Result<()>,
          H: FnMut(&ResponseHeader) -> Result<()>
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
        RedirectTemporary | RedirectPermanent => {
            return fetch_(&header.meta, config, reader_cb, doc_cb, header_cb, depth + 1);
        },

        Input | SensitiveInput => {
            let input = reader_cb(&header.meta, header.status == SensitiveInput)?;
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
            return fetch_(url.as_str(), config, reader_cb, doc_cb, header_cb, depth + 1);
        },
        // Only read the response body if we got a Success response status
        Success =>
            if header.meta.starts_with("text/gemini") {
                let body = std::str::from_utf8(body)?;
                println!("Got body:\n{}", body);
                let (_, text) = parse_text_gemini(body).map_err(
                    |e| anyhow!("text/gemini parsing failed: {}", e))?;
                doc_cb(&text)?;
            } else if header.meta.starts_with("text/") {
                // Read other text/ MIME types as a single preformatted line
                let body = std::str::from_utf8(body)?;
                let text = Line::Pre { alt: None, text: body };
                doc_cb(&Document(vec![text]))?;
            } else {
                return Err(anyhow!("Unknown meta: {}", header.meta));
            },

        // Otherwise, invoke the header cb
        _ => { header_cb(&header)?; }
    }
    Ok(())
}

////////////////////////////////////////////////////////////////////////////////

fn read_bytes(prompt: &str, _sensitive: bool) -> Result<String> {
    print!("{}", prompt);
    std::io::stdout().lock().flush()?;
    let mut buf = String::new();
    std::io::stdin().lock().read_line(&mut buf)?;
    Ok(buf)
}

fn print_doc(doc: &Document) -> Result<()> {
    println!("{:?}", doc);
    Ok(())
}

fn print_header(header: &ResponseHeader) -> Result<()> {
    println!("{:?}", header);
    Ok(())
}

////////////////////////////////////////////////////////////////////////////////

fn main() -> Result<()> {
    let dirs = directories::ProjectDirs::from("com", "mkeeter", "titan")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other,
                                           "Could not get ProjectDirs"))?;
    let db = sled::open(dirs.data_dir())?;

    let mut config = rustls::ClientConfig::new();
    let verifier = GeminiCertificateVerifier::new(&db)?;
    config.dangerous().set_certificate_verifier(Arc::new(verifier));
    let config = Arc::new(config);

    fetch("gemini://gemini.circumlunar.space", config, read_bytes, print_doc, print_header)?;
        /*
    println!("{:?}", doc);
    doc.word_wrap(60)
        .pretty_print();

    println!("{:?}", Document(vec![Line::Text("".to_string())]).word_wrap(40));
        */
    Ok(())
}
