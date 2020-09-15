use std::io::{Read, Write};
use std::sync::{Arc};
use std::net::TcpStream;

use anyhow::{anyhow, Result};

use crate::tofu::GeminiCertificateVerifier;
use crate::command::Command;
use crate::document::Document;
use crate::parser::{parse_response_header, parse_text_gemini};
use crate::protocol::{Line, ResponseHeader, ResponseStatus};
use crate::view::View;

pub struct App {
    config: Arc<rustls::ClientConfig>,
}

impl App {
    pub fn new(db: &sled::Db) -> Result<App> {
        let mut config = rustls::ClientConfig::new();
        let verifier = GeminiCertificateVerifier::new(&db)?;
        config.dangerous().set_certificate_verifier(Arc::new(verifier));
        let config = Arc::new(config);
        Ok(App { config })
    }

    pub fn run(&self, mut target: String) -> Result<()> {
        loop {
            match self.fetch(&target)? {
                Command::Exit => break Ok(()),
                Command::Load(s) => target = s,
                Command::Continue | Command::Unknown(_) => unreachable!(),
            }
        }
    }

    pub fn fetch(&self, target: &str) -> Result<Command> {
        self.fetch_(target, 0)
    }

    fn fetch_(&self, target: &str, depth: u8) -> Result<Command> {
        if depth >= 5 {
            return Err(anyhow!("Too much recursion"));
        }

        // TODO: handle relative links within a document
        let mut url = url::Url::parse(target);
        if url == Err(url::ParseError::RelativeUrlWithoutBase) {
            url = url::Url::parse(&format!("gemini://{}", target));
        }
        let url = url?;

        if url.scheme() != "gemini" {
            return Err(anyhow!("Invalid URL scheme: {}", url.scheme()));
        }

        let hostname = url.host_str()
            .ok_or_else(|| anyhow!("Error: no hostname in {}", url.as_str()))?;
        let dns_name = webpki::DNSNameRef::try_from_ascii_str(hostname)?;
        let mut sess = rustls::ClientSession::new(&self.config, dns_name);

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

        let (body, header) = parse_response_header(&plaintext).map_err(
            |e| anyhow!("Header parsing failed: {}", e))?;

        use ResponseStatus::*;
        match header.status {
            RedirectTemporary | RedirectPermanent => {
                self.fetch_(&header.meta, depth + 1)
            },

            Input | SensitiveInput => {
                let input = ""; // TODO
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
                self.fetch_(url.as_str(), depth + 1)
            },
            // Only read the response body if we got a Success response status
            Success => {
                // TODO: cb.header(&header)?;
                if header.meta.starts_with("text/gemini") {
                    let body = std::str::from_utf8(body)?;
                    let (_, doc) = parse_text_gemini(body).map_err(
                        |e| anyhow!("text/gemini parsing failed: {}", e))?;
                    self.display_doc(&doc)
                } else if header.meta.starts_with("text/") {
                    // Read other text/ MIME types as a single preformatted line
                    let body = std::str::from_utf8(body)?;
                    let text = Line::Pre { alt: None, text: body };
                    self.display_doc(&Document::new(vec![text]))
                } else {
                    Err(anyhow!("Unknown meta: {}", header.meta))
                }
            },

            // Otherwise, invoke the header cb
            _ => Ok(Command::Exit), // TODO cb.header(&header)?;
        }
    }

    fn display_doc(&self, doc: &Document) -> Result<Command> {
        let mut v = View::new(doc)?;
        loop {
            match v.run()? {
                Command::Continue => continue,
                Command::Unknown(cmd) => {
                    v.set_cmd_error(&format!("Unknown command: {}", cmd))?;
                },
                r => return Ok(r),
            }
        }
    }
}
