use std::io::{Read, Write};
use std::sync::{Arc};
use std::net::TcpStream;

use anyhow::{anyhow, Result};

use crate::tofu::GeminiCertificateVerifier;
use crate::command::Command;
use crate::document::Document;
use crate::input;
use crate::parser::{parse_response, parse_text_gemini};
use crate::protocol::{Line, ResponseStatus};
use crate::view::View;

use crossterm::{
    cursor,
    execute,
    terminal,
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{Clear, ClearType},
    style::{style, Color, Print, PrintStyledContent},
};

pub struct App {
    config: Arc<rustls::ClientConfig>,
    has_cmd_error: bool,
    size: (u16, u16), // width, height
}

impl App {
    pub fn new(db: &sled::Db) -> Result<App> {
        let mut config = rustls::ClientConfig::new();
        let verifier = GeminiCertificateVerifier::new(&db)?;
        config.dangerous().set_certificate_verifier(Arc::new(verifier));
        let config = Arc::new(config);
        let size = terminal::size()
            .expect("Could not get terminal size");
        Ok(App { config, has_cmd_error: false, size })
    }

    pub fn run(&mut self, mut target: url::Url) -> Result<()> {
        loop {
            // TODO: don't use a clone here?
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

    pub fn fetch(&mut self, url: url::Url) -> Result<Command> {
        self.fetch_(url, 0)
    }

    fn fetch_(&mut self, url: url::Url, depth: u8) -> Result<Command> {
        if depth >= 5 {
            return Err(anyhow!("Too much recursion"));
        }

        let plaintext = self.read(&url)?;
        let response = parse_response(&plaintext)?;

        use ResponseStatus::*;
        match response.status {
            RedirectTemporary | RedirectPermanent => {
                let next = url::Url::parse(response.meta)?;
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
                // TODO: Figure out how to draw the header
                if response.meta.starts_with("text/gemini") {
                    let body = std::str::from_utf8(response.body)?;
                    let (_, doc) = parse_text_gemini(body).map_err(
                        |e| anyhow!("text/gemini parsing failed: {}", e))?;
                    Ok(self.display_doc(&doc))
                } else if response.meta.starts_with("text/") {
                    // Read other text/ MIME types as a single preformatted line
                    let body = std::str::from_utf8(response.body)?;
                    let text = Line::Pre { alt: None, text: body };
                    Ok(self.display_doc(&Document::new(vec![text])))
                } else {
                    Err(anyhow!("Unknown meta: {}", response.meta))
                }
            },

            // Otherwise, invoke the header cb
            _ => Ok(Command::Exit), // TODO cb.header(&header)?;
        }
    }

    fn key(&mut self, k: KeyEvent) -> Option<Result<Command>> {
        // Exit on Ctrl-C, even though we don't get a true SIGINT
        if k.code == KeyCode::Char('c') &&
           k.modifiers == KeyModifiers::CONTROL
        {
            return Some(Ok(Command::Exit));
        }

        // Clear the command error pane on any keypress
        if self.has_cmd_error {
            self.clear_cmd();
        }

        // TODO: search mode with '/'
        // TODO: multiple up/down commands, e.g. 10j

        match k.code {
            KeyCode::Char(':') => {
                execute!(&mut std::io::stdout(),
                    cursor::MoveTo(0, self.size.1 + 1),
                    Print(":"),
                ).expect("Could not start drawing command line");
                if let Some(cmd) = input::Input::new().run() {
                    Some(Command::parse(cmd))
                } else {
                    self.clear_cmd();
                    None
                }
            },
            _ => None,
        }
    }

    fn set_cmd_error(&mut self, err: &str) {
        let mut out = std::io::stdout();
        execute!(&mut out,
            cursor::MoveTo(0, self.size.1 + 1),
            Clear(ClearType::CurrentLine),
            PrintStyledContent(style(err).with(Color::DarkRed)),
        ).expect("Failed to queue cmd error");
        self.has_cmd_error = true;
    }

    fn clear_cmd(&mut self) {
        let mut out = std::io::stdout();
        execute!(&mut out,
            cursor::MoveTo(0, self.size.1 + 1),
            Clear(ClearType::CurrentLine),
        ).expect("Failed to queue cmd clear");
        self.has_cmd_error = false;
    }

    fn event(&mut self, evt: Event) -> Option<Result<Command>> {
        match evt {
            Event::Key(event) => self.key(event),
            Event::Resize(w, h) => {
                self.resize((w, h));
                None
            },
            _ => None,
        }
    }

    fn resize(&mut self, size: (u16, u16)) {
        self.size = size;
    }

    fn display_doc(&mut self, doc: &Document) -> Command {
        let mut v = View::new(doc);
        loop {
            let evt = read().expect("Could not read event");

            // Handle some events ourselves, before possibly
            // passing them to the document view
            if let Some(r) = self.event(evt).or_else(|| v.event(evt)) {
                match r {
                    Err(err) => self.set_cmd_error(&format!("{}", err)),
                    Ok(r) => break r,
                }
            }
        }
    }
}
