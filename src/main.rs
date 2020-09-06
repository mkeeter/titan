use std::io::{BufRead, Write};
use std::sync::{Arc};

use anyhow::Result;

mod document;
mod protocol;
mod parser;
mod tofu;
mod fetch;
mod view;

use crate::document::Document;
use crate::fetch::{Fetch, fetch};
use crate::protocol::{ResponseHeader};
use crate::tofu::GeminiCertificateVerifier;

////////////////////////////////////////////////////////////////////////////////

struct Simple { }

impl Fetch for Simple {
    fn input(&mut self, prompt: &str, _is_sensitive: bool) -> Result<String> {
        print!("{}", prompt);
        std::io::stdout().lock().flush()?;
        let mut buf = String::new();
        std::io::stdin().lock().read_line(&mut buf)?;
        Ok(buf)
    }

    fn display(&mut self, doc: &Document) -> Result<()> {
        doc.word_wrap(40).pretty_print();
        Ok(())
    }
    fn header(&mut self, header: &ResponseHeader) -> Result<()> {
        println!("Response header: {:?}", header);
        Ok(())
    }
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

    fetch("gemini://gemini.circumlunar.space", config, &mut Simple{})?;
    Ok(())
}
