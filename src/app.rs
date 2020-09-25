use std::io::{Read, Write};
use std::sync::{Arc};
use std::net::TcpStream;

use anyhow::{anyhow, Result};

use crate::tofu::GeminiCertificateVerifier;
use crate::command::Command;
use crate::document::Document;
use crate::input;
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

    pub fn run(&self, mut target: url::Url) -> Result<()> {
        loop {
            // TODO: don't use a reference here?
            match self.fetch(target.clone())? {
                Command::Exit => break Ok(()),
                Command::Load(s) => target = s,
                Command::TryLoad(s) => {
                    let mut url = url::Url::parse(&s);
                    if url == Err(url::ParseError::RelativeUrlWithoutBase) {
                        url = target.join(&s);
                    }
                    match url {
                        // TODO: how to display error here?
                        Err(e) => continue,
                        Ok(url) => target = url,
                    }
                },
            }
        }
    }

    fn read(&self, url: &url::Url) -> Result<Vec<u8>> {
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
        Ok(plaintext)
    }

    pub fn fetch(&self, url: url::Url) -> Result<Command> {
        self.fetch_(url, 0)
    }

    fn fetch_(&self, url: url::Url, depth: u8) -> Result<Command> {
        if depth >= 5 {
            return Err(anyhow!("Too much recursion"));
        }

        let plaintext = self.read(&url)?;

        let (body, header) = parse_response_header(&plaintext)
            .map_err(|e| anyhow!("Header parsing failed: {}", e))?;

        use ResponseStatus::*;
        match header.status {
            RedirectTemporary | RedirectPermanent => {
                let next = url::Url::parse(header.meta)?;
                self.fetch_(next, depth + 1)
            },

            Input | SensitiveInput => {
                if let Some(input) = input::Input::new().run() {
                    // Serialize the input string and set it as the query param
                    use url::form_urlencoded::byte_serialize;
                    let input: String = byte_serialize(input.as_bytes())
                        .collect();

                    let mut url = url;
                    url.set_query(Some(&input));
                    self.fetch_(url, depth + 1)
                } else {
                    Err(anyhow!("Failed to get input"))
                }
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
        let mut v = View::new(doc);
        loop {
            match v.run() {
                Ok(cmd) => return Ok(cmd),
                Err(err) => v.set_cmd_error(&format!("{}", err)),
            }
        }
    }
}
