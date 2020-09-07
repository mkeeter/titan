use std::convert::TryInto;
use std::io::{Write};

use crate::document::{Document, WrappedDocument};
use crate::protocol::{ResponseHeader, Line_};
use crate::fetch::Fetch;

use anyhow::Result;
use crossterm::{
    cursor,
    execute,
    terminal,
    event::{read, Event, KeyCode, KeyModifiers},
    terminal::{Clear, ClearType},
    style::{style, Attribute, Color, Print, PrintStyledContent, StyledContent},
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

    fn style_text(s: &str) -> StyledContent<&str> {
        style(s)
    }
    fn style_h1(s: &str) -> StyledContent<&str> {
        style(s).with(Color::DarkRed)
    }
    fn style_h2(s: &str) -> StyledContent<&str> {
        style(s).with(Color::DarkYellow)
    }
    fn style_h3(s: &str) -> StyledContent<&str> {
        style(s).with(Color::DarkCyan)
    }
    fn style_pre(s: &str) -> StyledContent<&str> {
        style(s).with(Color::Red)
    }
    fn style_list(s: &str) -> StyledContent<&str> {
        style(s)
    }
    fn style_quote(s: &str) -> StyledContent<&str> {
        style(s).with(Color::White)
    }
    fn style_link(s: &str) -> StyledContent<&str> {
        style(s).with(Color::Magenta)
    }

    // Draws a block of lines starting at a given y position and either
    // filling the screen or finishing the block.
    //
    // Returns the number of lines that has been output
    fn draw_block<W: Write>(&self, out: &mut W, lines: &[&str], y: u16,
                            f: &dyn Fn(&str) -> StyledContent<&str>,
                            first: &str, later: &str)
        -> Result<usize>
    {
        let dy = self.size.0 - y; // Max number of lines to draw
        for (i, line) in lines.iter().take(dy as usize).enumerate() {
            queue!(out,
                cursor::MoveTo(0, y + i as u16),
                Print(if i == 0 { first } else { later }),
                PrintStyledContent(f(line)))?;
        }
        Ok((dy as usize).min(lines.len()).max(1))
    }

    fn draw_line<W: Write>(&self, out: &mut W, line: usize, y: u16, slice: usize) -> Result<usize> {
        use Line_::*;

        let line = &self.doc.0[line];
        if let Link { name: None, url } = line {
            assert!(slice == 0);
            return self.draw_block(out, &[url], y, &Self::style_link, "→ ", "  ");
        }

        // We trust that the line-wrapping has wrapped things like quotes and
        // links so that there's room for their prefixes here.  We have to do
        // a little persuasion here to convince the type system to accept our
        // styling functions
        let (v, mut first, later, f):
            (_, _, _, &dyn Fn(&str) -> StyledContent<&str>) = match line
        {
            Text(t) => (t, "", "", &Self::style_text),
            Link { name: Some(t), .. } => (t, "→ ", "  ", &Self::style_link),
            Pre { text, .. } => (text, "", "", &Self::style_pre),
            H1(t) => (t, "# ", "  ", &Self::style_h1),
            H2(t) => (t, "## ", "   ", &Self::style_h2),
            H3(t) => (t, "### ", "    ", &Self::style_h3),
            List(t) => (t, "• ", "  ", &Self::style_list),
            Quote(t) => (t, "> ", "> ", &Self::style_quote),
            _ => unreachable!(),
        };

        if slice > 0 {
            first = later;
        }
        self.draw_block(out, &v[slice..], y, f, first, later)
    }

    fn draw(&self) -> Result<()> {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();

        queue!(out, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

        // Draw the first block, which could be partial
        let mut y = self.draw_line(&mut out, self.pos.0, 0, self.pos.1)?;

        // Then draw as many other blocks as will fit
        let mut i = 0;
        while y < self.size.1.into() && i + 1 < self.doc.0.len() {
            i += 1;
            y += self.draw_line(&mut out, i, y.try_into().unwrap(), 0)?;
        }
        queue!(out, cursor::Hide)?;

        out.flush()?;
        Ok(())
    }

    fn down(&self) -> Result<()> {
        self.draw()
    }
    fn up(&self) -> Result<()> {
        self.draw()
    }
}

impl Fetch for View {
    fn input(&mut self, _prompt: &str, _is_sensitive: bool) -> Result<String> {
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
                    match event.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('j') => view.down()?,
                        KeyCode::Char('k') => view.up()?,
                        KeyCode::Char('c') => if event.modifiers == KeyModifiers::CONTROL {
                            break;
                        },
                        _ => (),
                    }
                },
                _ => (),
            }
        }
        execute!(std::io::stdout(), cursor::Show)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    fn header(&mut self, _header: &ResponseHeader) -> Result<()> {
        unimplemented!("No header function yet");
    }
}
