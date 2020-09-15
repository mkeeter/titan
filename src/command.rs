#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Continue,
    Exit,
    Load(url::Url),
    TryLoad(String),
    Unknown(String),
    Error(String),
}

impl Command {
    pub fn parse(cmd: String) -> Command {
        // TODO: use nom here as well
        let mut itr = cmd.split_whitespace();
        if let Some(c) = itr.next() {
            match c {
                "q" => Command::Exit,
                "g" => if let Some(t) = itr.next() {
                    let mut url = url::Url::parse(t);
                    if url == Err(url::ParseError::RelativeUrlWithoutBase) {
                        url = url::Url::parse(&format!("gemini://{}", t));
                    }
                    match url {
                        Ok(url) => Command::Load(url),
                        Err(e) => Command::Error(format!("Invalid URL {}", t)),
                    }
                } else {
                    Command::Unknown("Missing URL".to_string())
                },
                _ => Command::Unknown(cmd),
            }
        } else {
            Command::Unknown(cmd)
        }
    }
}
