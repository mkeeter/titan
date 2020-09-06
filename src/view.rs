use crate::document::Document;
use crate::protocol::{ResponseHeader};

use crate::fetch::Fetch;

use anyhow::Result;

struct View();

impl Fetch for View {
    fn input(&mut self, prompt: &str, is_sensitive: bool) -> Result<String> {
        unimplemented!("No input function yet");
    }
    fn display(&mut self, doc: &Document) -> Result<()> {
        unimplemented!("No view function yet");
    }
    fn header(&mut self, header: &ResponseHeader) -> Result<()> {
        unimplemented!("No header function yet");
    }
}
