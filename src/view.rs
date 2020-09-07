use std::convert::TryInto;
use std::io::{Write};
use std::fmt::Display;

use crate::document::{Document, WrappedDocument};
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

    // Draws a block of lines starting at a given y position and either
    // filling the screen or finishing the block.
    //
    // Returns the number of lines that has been output
    fn draw_block<W: Write>(&self, out: &mut W, lines: &[&str], y: u16,
                            first: &str, later: &str)
        -> Result<usize>
    {
        let dy = self.size.0 - y; // Max number of lines to draw
        for (i, line) in lines.iter().take(dy as usize).enumerate() {
            queue!(out,
                cursor::MoveTo(0, y + i as u16),
                Print(if i == 0 { first } else { later }),
                Print(line))?;
        }
        Ok((dy as usize).min(lines.len()).max(1))
    }

    fn draw_line<W: Write>(&self, out: &mut W, line: usize, y: u16, slice: usize) -> Result<usize> {
        use Line_::*;

        if let Link { name: None, url } = &self.doc.0[line] {
        }

        // We trust that the line-wrapping has wrapped things like quotes and
        // links so that there's room for their prefixes here.
        let (v, mut first, later) = match &self.doc.0[line] {
            Text(t) => (t, "", ""),
            Link { name: Some(t), .. } => (t, "=> ", "   "),
            Pre { text, .. } => (text, "", ""),
            H1(t) => (t, "# ", "  "),
            H2(t) => (t, "## ", "   "),
            H3(t) => (t, "### ", "    "),
            List(t) => (t, "• ", "  "),
            Quote(t) => (t, "> ", "> "),
            _ => unreachable!(),
        };

        if slice > 0 {
            first = later;
        }
        self.draw_block(out, &v[slice..], y, first, later)
    }

    fn draw(&self) -> Result<()> {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();

        queue!(out, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

        // Draw the first block, which could be partial
        let mut y = self.draw_line(&mut out, self.pos.0, 0, self.pos.1)?;

        // Then draw as many other blocks as will fit
        let mut i = 0;
        while y < self.size.1.into() {
            i += 1;
            y += self.draw_line(&mut out, i, y.try_into().unwrap(), 0)?;
        }
        queue!(out, cursor::Hide)?;

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
        view.draw()?;
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
