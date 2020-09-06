use std::io::{Write};
use std::fmt::Display;

use crate::document::Document;
use crate::protocol::{ResponseHeader, Line_};
use crate::fetch::Fetch;

use anyhow::Result;
use crossterm::{
    cursor,
    terminal,
    event::{read, Event, KeyCode},
    terminal::{Clear, ClearType},
    style::{Print},
    queue,
};

pub struct View {
    out: std::io::Stdout,
}

impl View {
    pub fn new() -> View {
        View { out: std::io::stdout() }
    }

    fn pprint<T: Display + Clone>(&mut self, lines: &[T]) -> Result<()> {
        if lines.is_empty() {
            queue!(self.out, cursor::MoveDown(0))?;
        } else {
            for line in lines.iter() {
                queue!(self.out, Print(line),
                       cursor::MoveDown(0), cursor::MoveToColumn(0))?;
            }
        }
        Ok(())
    }
}

impl Fetch for View {
    fn input(&mut self, prompt: &str, is_sensitive: bool) -> Result<String> {
        unimplemented!("No input function yet");
    }

    fn display(&mut self, doc: &Document) -> Result<()> {
        terminal::enable_raw_mode()?;
        let (tw, th) = terminal::size()?;
        let d = doc.word_wrap(tw.into());
        queue!(self.out, cursor::MoveTo(0, 0), Clear(ClearType::All))?;
        for block in d.0 {
            use Line_::*;
            match block {
                Text(t) => self.pprint(&t)?,
                Link { name: Some(name), .. } => self.pprint(&name)?,
                Link { name: None, url } => self.pprint(&[url])?,
                Pre { text, .. } => self.pprint(&text)?,
                H1(t) => self.pprint(&t)?,
                H2(t) => self.pprint(&t)?,
                H3(t) => self.pprint(&t)?,
                List(t) => self.pprint(&t)?,
                Quote(t) => self.pprint(&t)?,
            }
        }
        self.out.flush()?;
        loop {
            // `read()` blocks until an `Event` is available
            let evt = read()?;
            println!("Got event {:?}", evt);
            match evt {
                Event::Key(event) => {
                    if event.code == KeyCode::Char('q') {
                        break;
                    }
                },
                _ => (),
            }
        }
        Ok(())
    }
    fn header(&mut self, header: &ResponseHeader) -> Result<()> {
        unimplemented!("No header function yet");
    }
}
