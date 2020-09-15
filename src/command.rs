#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Continue,
    Exit,
    Load(String),
    Unknown(String),
}
