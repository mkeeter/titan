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
    event,
    event::{read, Event, KeyCode, KeyModifiers, MouseEvent},
    terminal::{Clear, ClearType},
    style::{style, Color, ContentStyle, Print, PrintStyledContent},
    queue,
};

pub struct View { }

struct WrappedView<'a> {
    size: (u16, u16), // width, height
    yscroll: (usize, usize), // Y scoll position in the doc (block, line)
    ycursor: (usize, usize), // Y cursor position in the doc (block, line)
    doc: WrappedDocument<'a>,
}

impl WrappedView<'_> {
    fn new<'a>(doc: &'a Document, size: (u16, u16), yscroll: (usize, usize))
        -> WrappedView<'a>
    {
        let doc = doc.word_wrap((size.0 - 1).into());
        WrappedView { doc, size, yscroll, ycursor: yscroll }
    }

    // Draws a block of lines starting at a given y position and either
    // filling the screen or finishing the block.
    //
    // Returns the number of lines that has been output
    fn draw_block<W: Write>(&self, out: &mut W, lines: &[&str], sy: u16,
                            color: &ContentStyle,
                            first: &str, later: &str,
                            active_block: bool, active_line: usize)
        -> Result<usize>
    {
        let dy = self.size.0 - sy; // Max number of lines to draw
        for (i, line) in lines.iter().take(dy as usize).enumerate() {
            if active_block {
                queue!(out,
                    cursor::MoveTo(0, sy + i as u16),
                    PrintStyledContent(style(" ").on(Color::Black)))?;
                if i == active_line {
                    let fill = " ".repeat((self.size.0 - 1).into());
                    queue!(out,
                        PrintStyledContent(style(fill).on(Color::Black)),
                        cursor::MoveTo(1, sy + i as u16))?;
                }
            } else {
                queue!(out,
                    cursor::MoveTo(1, sy + i as u16))?;
            }

            if active_block && i == active_line {
                let color = color.clone().background(Color::Black);
                queue!(out,
                    PrintStyledContent(color.clone().apply(
                            if i == 0 { first } else { later })),
                    PrintStyledContent(color.apply(line)))?;
            } else {
                queue!(out,
                    Print(if i == 0 { first } else { later }),
                    PrintStyledContent(color.clone().apply(line)))?;
            };

        }
        Ok((dy as usize).min(lines.len()))
    }

    // Draws the block slice at the given index, starting at screen y pos sy
    fn draw_line<W: Write>(&self, out: &mut W, index: (usize, usize), sy: u16)
            -> Result<usize>
    {
        use Line_::*;
        let c = ContentStyle::new();

        // Special-case for URLs without alt text, which are drawn on
        // a single line.  TODO: handle overly long lines here.
        let line = &self.doc.0[index.0];
        if let Link { name: None, url } = line {
            assert!(index.1 == 0);
            return self.draw_block(
                out, &[url], sy, &c.foreground(Color::Magenta),
                "→ ", "  ", self.ycursor.0 == 0, 0);
        }

        // We trust that the line-wrapping has wrapped things like quotes and
        // links so that there's room for their prefixes here.  We have to do
        // a little persuasion here to convince the type system to accept our
        // styling functions
        let (v, mut first, later, style) = match line {
            Text(t) => (t, "", "", c),
            Link { name: Some(t), .. } => (t, "→ ", "  ", c.foreground(Color::Magenta)),
            H1(t) => (t, "# ", "  ", c.foreground(Color::DarkRed)),
            H2(t) => (t, "## ", "   ", c.foreground(Color::DarkYellow)),
            H3(t) => (t, "### ", "    ", c.foreground(Color::DarkCyan)),
            List(t) => (t, "• ", "  ", c),
            Quote(t) => (t, "> ", "> ", c.foreground(Color::White)),

            // TODO: handle overly long Pre lines
            Pre { text, .. } => (text, "", "", c.foreground(Color::Red)),

            _ => unreachable!(),
        };

        // If this is a partial slice, then don't draw first-line-only
        // text decoration (e.g. "→" for links)
        if index.1 > 0 {
            first = later;
        }
        self.draw_block(out, &v[index.1..], sy, &style, first, later,
                        index.0 == self.ycursor.0,
                        self.ycursor.1 - index.1)
    }

    fn draw(&self) -> Result<()> {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();

        queue!(out, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

        // Draw the first block, which could be partial
        let mut y = self.draw_line(&mut out, self.yscroll, 0)?;

        // Then draw as many other blocks as will fit
        let mut i = 0;
        while y < self.size.1.into() && i + 1 < self.doc.0.len() {
            i += 1;
            y += self.draw_line(&mut out, (i, 0), y.try_into().unwrap())?;
        }

        out.flush()?;
        Ok(())
    }

    fn down(&mut self) {
        // End of block
        if self.ycursor.1 == self.doc.0[self.ycursor.0].len() - 1 {
            if self.ycursor.0 == self.doc.0.len() - 1 {
                // End of doc
            } else {
                self.ycursor.0 += 1;
                self.ycursor.1 = 0;
            }
        } else {
            self.ycursor.1 += 1;
        }
    }

    fn up(&mut self) {
        // Beginning of block
        if self.ycursor.1 == 0 {
            if self.ycursor.0 == 0 {
                // Beginning of doc
            } else {
                // Previous block
                self.ycursor.0 -= 1;
                self.ycursor.1 = self.doc.0[self.ycursor.0].len() - 1;
            }
        } else {
            // Previous line
            self.ycursor.1 -= 1;
        }
    }
}

impl Fetch for View {
    fn input(&mut self, _prompt: &str, _is_sensitive: bool) -> Result<String> {
        unimplemented!("No input function yet");
    }

    fn display(&mut self, doc: &Document) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(std::io::stdout(), cursor::Hide, event::EnableMouseCapture)?;
        let (tw, th) = terminal::size()?;
        let mut view = WrappedView::new(doc, (tw, th), (0, 0));
        view.draw()?;
        loop {
            // `read()` blocks until an `Event` is available
            let evt = read()?;
            match evt {
                Event::Key(event) => {
                    match event.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('j') => {
                            view.down();
                            view.draw()?;
                        },
                        KeyCode::Char('k') => {
                            view.up();
                            view.draw()?;
                        },
                        KeyCode::Char('c') =>
                            // Quit on Control-C, even though it's not
                            // actually coming through as an interrupt.
                            if event.modifiers == KeyModifiers::CONTROL {
                                break;
                            },
                        _ => (),
                    }
                },
                Event::Mouse(event) => {
                    match event {
                        MouseEvent::ScrollUp(..) => {
                            view.up();
                            view.draw()?;
                        },
                        MouseEvent::ScrollDown(..) => {
                            view.down();
                            view.draw()?;
                        },
                        _ => (),
                    }
                },
                _ => (),
            }
        }
        execute!(std::io::stdout(), cursor::Show,
                 event::DisableMouseCapture)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    fn header(&mut self, _header: &ResponseHeader) -> Result<()> {
        unimplemented!("No header function yet");
    }
}
