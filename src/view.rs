use std::convert::TryInto;
use std::io::{Write};

use crate::document::{Document, WrappedDocument};
use crate::protocol::{ResponseHeader, Line_};
use crate::fetch::Fetch;

use anyhow::{Result};
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
    source: &'a Document<'a>,
    doc: WrappedDocument<'a>,
    offsets: Vec<usize>, // cumsum of lines in doc.0
    needs_redraw: bool,
}

impl WrappedView<'_> {
    fn new<'a>(source: &'a Document, size: (u16, u16),
               yscroll: usize, ycursor: usize) -> WrappedView<'a>
    {
        // Add two characters of padding on either side
        let tw = size.0 - 4;
        let doc = source.word_wrap((size.0 - 4).into());

        // Add a status and command bar at the bottom
        let th = size.1 - 2;

        let offsets = doc.0.iter()
            .scan(0, |i, j| {
                let out = *i;
                *i += j.len();
                Some(out)
            })
            .collect();
        WrappedView { doc, offsets, source,
            size: (tw, th),
            ycursor: (ycursor, 0),
            yscroll: (yscroll, 0),
            needs_redraw: true}
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
        let dy = self.size.1 - sy; // Max number of lines to draw
        for (i, line) in lines.iter().take(dy as usize).enumerate() {
            if active_block {
                queue!(out,
                    cursor::MoveTo(0, sy + i as u16),
                    PrintStyledContent(style(" ").on(Color::Black)))?;
                if i == active_line {
                    let fill = " ".repeat((self.size.0 + 1).into());
                    queue!(out,
                        PrintStyledContent(style(fill).on(Color::Black)),
                    )?;
                }
                queue!(out, cursor::MoveTo(2, sy + i as u16))?;
            } else {
                queue!(out,
                    cursor::MoveTo(2, sy + i as u16))?;
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

        queue!(out,
            cursor::MoveTo(self.size.0 + 4, self.size.1 - 1),
            Clear(ClearType::FromCursorUp),
        )?;

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
        let prev = (self.ycursor, self.yscroll);
        self.ycursor = self.increment_index(self.ycursor);

        // If we've scrolled off the bottom of the screen, then adjust the
        // scroll position as well
        if self.cursor_line() >= self.scroll_line() + self.size.1 as usize {
            self.yscroll = self.increment_index(self.yscroll);
        }
        self.needs_redraw = prev != (self.ycursor, self.yscroll);
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
        let prev = (self.ycursor, self.yscroll);
        self.ycursor = self.decrement_index(self.ycursor);
        if self.cursor_line() < self.scroll_line() {
            self.yscroll = self.decrement_index(self.yscroll);
        }
        self.needs_redraw = prev != (self.ycursor, self.yscroll);
    }
}

impl View {
    fn event(&mut self, evt: Event, view: &mut WrappedView) -> Result<bool> {
        match evt {
            Event::Key(event) => {
                match event.code {
                    KeyCode::Char('q') => { return Ok(false); }
                    KeyCode::Char('j') => view.down(),
                    KeyCode::Char('k') => view.up(),
                    KeyCode::Char('c') =>
                        // Quit on Control-C, even though it's not
                        // actually coming through as an interrupt.
                        if event.modifiers == KeyModifiers::CONTROL {
                            return Ok(false);
                        }
                    _ => (),
                }
            },
            Event::Mouse(event) => {
                match event {
                    MouseEvent::ScrollUp(..) => view.up(),
                    MouseEvent::ScrollDown(..) => view.down(),
                    _ => (),
                }
            },
            Event::Resize(w, h) => {
                *view = WrappedView::new(view.source, (w, h),
                                         view.yscroll.0, view.ycursor.0);
            },
        }
        Ok(true)
    }
}

impl Fetch for View {
    fn input(&mut self, _prompt: &str, _is_sensitive: bool) -> Result<String> {
        unimplemented!("No input function yet");
    }

    fn display(&mut self, doc: &Document) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(std::io::stdout(), cursor::Hide, event::EnableMouseCapture)?;
        let mut view = WrappedView::new(doc, terminal::size()?, 0, 0);
        view.draw()?;

        loop {
            let evt = read()?;
            if !self.event(evt, &mut view)? {
                break;
            }
            if view.needs_redraw {
                view.draw()?;
                view.needs_redraw = false;
            }
        }
        execute!(std::io::stdout(), cursor::Show,
                 event::DisableMouseCapture)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    fn header(&mut self, header: &ResponseHeader) -> Result<()> {
        let (_, th) = terminal::size()?;
        let mut out = std::io::stdout();
        let s = format!(" {:?}: {} ", header.status, header.meta);
        queue!(out,
            cursor::MoveTo(0, th - 2),
            terminal::Clear(ClearType::FromCursorDown),
            PrintStyledContent(style(s).with(Color::Black).on(Color::Blue)),
        )?;
        out.flush()?;
        Ok(())
    }
}
