use anyhow::{anyhow, Result};

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Exit,
    Load(url::Url),
    TryLoad(String),
}

impl Command {
    pub fn parse(cmd: String) -> Result<Command> {
        // TODO: use nom here as well
        let mut itr = cmd.split_whitespace();
        if let Some(c) = itr.next() {
            match c {
                "q" => Ok(Command::Exit),
                "g" => if let Some(t) = itr.next() {
                    let mut url = url::Url::parse(t);
                    if url == Err(url::ParseError::RelativeUrlWithoutBase) {
                        url = url::Url::parse(&format!("gemini://{}", t));
                    }
                    match url {
                        Ok(url) => Ok(Command::Load(url)),
                        Err(e) => Err(anyhow!("Invalid URL {}", t)),
                    }
                } else {
                    Err(anyhow!("Missing URL"))
                },
                _ => Err(anyhow!("Unknown command: {}", cmd))
            }
        } else {
            Err(anyhow!("Unknown command: {}", cmd))
        }
    }
}
