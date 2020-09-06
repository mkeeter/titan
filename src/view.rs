use std::convert::TryInto;
use std::io::{Write};
use std::fmt::Display;

use crate::document::{Document, WrappedDocument, WrappedLine};
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

pub struct View { }

struct WrappedView<'a> {
    size: (u16, u16), // width, height
    pos: (usize, usize), // Y position in the doc (block, line)
    doc: WrappedDocument<'a>,
}

impl WrappedView<'_> {
    fn new<'a>(doc: &'a Document, size: (u16, u16), pos: (usize, usize))
        -> WrappedView<'a>
    {
        let doc = doc.word_wrap(size.0.into());

        WrappedView { doc, size, pos: (pos.0, 0) }
    }

    fn draw_block<T: Display + Clone>(&self, out: &mut std::io::Stdout, lines: &[T], y: u16)
        -> Result<usize>
    {
        let dy = self.size.0 - y; // Max number of lines to draw
        for (i, line) in lines.iter().take(dy as usize).enumerate() {
            queue!(out, cursor::MoveTo(0, y + i as u16), Print(line))?;
        }
        Ok((dy as usize).min(lines.len()).max(1))
    }

    fn draw_line(&self, out: &mut std::io::Stdout, line: usize, y: u16, slice: usize) -> Result<usize> {
        use Line_::*;
        match &self.doc.0[line] {
            Text(t) => self.draw_block(out, &t[slice..], y),
            Link { name: Some(t), .. } => self.draw_block(out, &t[slice..], y),
            Link { name: None, url } => self.draw_block(out, &[url], y),
            Pre { text, .. } => self.draw_block(out, &text[slice..], y),
            H1(t) => self.draw_block(out, &t[slice..], y),
            H2(t) => self.draw_block(out, &t[slice..], y),
            H3(t) => self.draw_block(out, &t[slice..], y),
            List(t) => self.draw_block(out, &t[slice..], y),
            Quote(t) => self.draw_block(out, &t[slice..], y),
        }
    }

    fn draw(&self) -> Result<()> {
        let mut out = std::io::stdout();

        queue!(out, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

        // Draw the first block, which could be partial
        let mut y = self.draw_line(&mut out, self.pos.0, 0, self.pos.1)?;

        // Then draw as many other blocks as will fit
        let mut i = 0;
        while y < self.size.1.into() {
            i += 1;
            y += self.draw_line(&mut out, i, y.try_into().unwrap(), 0)?;
        }

        out.flush()?;
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
        let view = WrappedView::new(doc, (tw, th), (0, 0));
        view.draw();
        loop {
            // `read()` blocks until an `Event` is available
            let evt = read()?;
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
