use std::sync::{Arc};

use anyhow::Result;

mod document;
mod protocol;
mod parser;
mod tofu;
mod fetch;
mod view;

use crate::fetch::fetch;
use crate::tofu::GeminiCertificateVerifier;
use crate::view::View;

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

    fetch("gemini://gemini.circumlunar.space", config, &mut View::new())?;
    Ok(())
}
