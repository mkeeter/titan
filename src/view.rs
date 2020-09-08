use std::convert::TryInto;
use std::io::{Write};

use crate::document::{Document, WrappedDocument};
use crate::protocol::{ResponseHeader, Line_};
use crate::fetch::Fetch;

use anyhow::{anyhow, Result};
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
    offsets: Vec<usize>, // cumsum of lines in doc.0
}

impl WrappedView<'_> {
    fn new<'a>(doc: &'a Document, size: (u16, u16), yscroll: (usize, usize))
        -> WrappedView<'a>
    {
        let doc = doc.word_wrap((size.0 - 1).into());
        let offsets = doc.0.iter()
            .scan(0, |i, j| {
                let out = *i;
                *i += j.len();
                Some(out)
            })
            .collect();
        WrappedView { doc, size, yscroll, ycursor: yscroll, offsets }
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
        queue!(out,
            cursor::MoveTo(0, self.size.1 - 1),
            Print(format!("{:?} {}, {:?} {}, {:?}",
                    self.ycursor, self.offsets[self.ycursor.0],
                    self.yscroll, self.offsets[self.yscroll.0],
                    self.size))
        )?;
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
        if let BareLink(url) = line {
            assert!(index.1 == 0);
            return self.draw_block(
                out, &[url], sy, &c.foreground(Color::Magenta),
                "→ ", "  ", self.ycursor.0 == 0, 0);
        }
        assert!(index.1 < line.len());

        // We trust that the line-wrapping has wrapped things like quotes and
        // links so that there's room for their prefixes here.  We have to do
        // a little persuasion here to convince the type system to accept our
        // styling functions
        let (v, mut first, later, style) = match line {
            Text(t) => (t, "", "", c),
            NamedLink { name, .. } => (name, "→ ", "  ", c.foreground(Color::Magenta)),
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
                        self.ycursor.1.saturating_sub(index.1))
    }

    fn draw(&self) -> Result<()> {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();

        queue!(out, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

        // Draw the first block, which could be partial
        let mut y = self.draw_line(&mut out, self.yscroll, 0)?;

        // Then draw as many other blocks as will fit
        let mut i = 0;
        while y < self.size.1.into() && self.yscroll.0 + i + 1 < self.doc.0.len() {
            i += 1;
            y += self.draw_line(&mut out, (self.yscroll.0 + i, 0), y.try_into().unwrap())?;
        }

        out.flush()?;
        Ok(())
    }

    // Safely increments a block/line index
    fn increment_index(&self, index: (usize, usize)) -> (usize, usize) {
        if index.1 == self.doc.0[index.0].len() - 1 {
            if index.0 == self.doc.0.len() - 1 {
                index
            } else {
                (index.0 + 1, 0)
            }
        } else {
            (index.0, index.1 + 1)
        }
    }

    fn cursor_line(&self) -> usize {
        self.offsets[self.ycursor.0] + self.ycursor.1
    }
    fn scroll_line(&self) -> usize {
        self.offsets[self.yscroll.0] + self.yscroll.1
    }

    fn down(&mut self) {
        self.ycursor = self.increment_index(self.ycursor);

        // If we've scrolled off the bottom of the screen, then adjust the
        // scroll position as well
        if self.cursor_line() >= self.scroll_line() + self.size.1 as usize - 1
        {
            self.yscroll = self.increment_index(self.yscroll);
        }
    }

    fn decrement_index(&self, index: (usize, usize)) -> (usize, usize) {
        if index.1 == 0 {
            if index.0 == 0 {
                index
            } else {
                (index.0 - 1, self.doc.0[index.0 - 1].len() - 1)
            }
        } else {
            (index.0, index.1 - 1)
        }
    }

    fn up(&mut self) {
        self.ycursor = self.decrement_index(self.ycursor);
        if self.cursor_line() < self.scroll_line() {
            self.yscroll = self.decrement_index(self.yscroll);
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

    fn header(&mut self, header: &ResponseHeader) -> Result<()> {
        println!("Got header: {:?}", header);
        Err(anyhow!("No header implementation yet"))
    }
}
