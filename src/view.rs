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
    source: &'a Document<'a>,
    doc: WrappedDocument<'a>,

    size: (u16, u16), // width, height

    yscroll: usize, // Y scoll position in the doc
    ycursor: usize, // Y cursor position in the doc
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

        WrappedView { doc, source, ycursor, yscroll,
            size: (tw, th),
        }
    }

    fn resize(&mut self, size: (u16, u16)) -> Result<()> {
        // Attempt to maintain roughly the same scroll and cursor position
        // after resizing is complete
        let yscroll_frac = self.yscroll as f32 / self.doc.0.len() as f32;
        let ycursor_frac = self.ycursor as f32 / self.doc.0.len() as f32;

        self.doc = self.source.word_wrap((size.0 - 4).into());
        self.size = (size.0 - 4, size.1 - 2);

        let dl = self.doc.0.len();
        self.ycursor = ((ycursor_frac * dl as f32) as usize)
            .max(0)
            .min(dl)
            .min((self.yscroll + self.size.1 as usize).saturating_sub(1));
        self.yscroll = ((yscroll_frac * dl as f32) as usize).max(0)
            .min(dl);

        self.draw()
    }

    // Calculates the text and prefix for a given line, which is given as its
    // text and a boolean indicating whether it's the first in its block.
    fn prefix<'a>(p: (&'a str, bool), first: &'static str, later: &'static str)
        -> (&'a str, &'static str)
    {
        (p.0, if p.1 { first } else { later })
    }

    fn draw_line<W: Write>(&self, out: &mut W, index: usize) -> Result<()> {
        // We trust that the line-wrapping has wrapped things like quotes and
        // links so that there's room for their prefixes here.
        let p = Self::prefix;

        use Line_::*;
        let c = ContentStyle::new();
        let ((text, prefix), c) = match self.doc.0[index] {
            Text(t) => ((t.0, ""), c),
            H1(t) => (p(t, "# ", "  "), c.foreground(Color::DarkRed)),
            H2(t) => (p(t, "## ", "   "), c.foreground(Color::DarkYellow)),
            H3(t) => (p(t, "### ", "    "), c.foreground(Color::DarkCyan)),
            List(t) => (p(t, "• ", "  "), c),
            Quote(t) => ((t.0, "> "), c.foreground(Color::White)),
            NamedLink { name, .. } => (p(name, "→ ", "  "),
                                       c.foreground(Color::Magenta)),

            // TODO: handle overly long Pre and BareLink lines
            BareLink(url) => ((url, "→ "), c.foreground(Color::Magenta)),
            Pre { text, .. } => ((text.0, ""), c.foreground(Color::Red)),
        };

        let sy = (index - self.yscroll).try_into().unwrap();
        assert!(sy < self.size.1);

        if index == self.ycursor {
            let c = c.background(Color::Black);
            let fill = " ".repeat((self.size.0 + 1).into());
            queue!(out,
                cursor::MoveTo(0, sy),
                PrintStyledContent(style(fill).on(Color::Black)),
                cursor::MoveTo(2, sy),
                PrintStyledContent(style(prefix).on(Color::Black)),
                PrintStyledContent(c.apply(text)),
            )?;
        } else {
            queue!(out,
                cursor::MoveTo(2, sy),
                Print(prefix),
                PrintStyledContent(c.apply(text)),
            )?;
        }
        Ok(())
    }

    fn draw(&self) -> Result<()> {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();

        queue!(out,
            cursor::MoveTo(self.size.0 + 4, self.size.1 - 1),
            Clear(ClearType::FromCursorUp),
        )?;

        for i in (0..self.size.1)
            .map(|i| i as usize + self.yscroll)
            .take_while(|i| *i < self.doc.0.len())
        {
            self.draw_line(&mut out, i)?;
        }

        out.flush()?;
        Ok(())
    }

    // Safely increments a line index
    fn increment_index(&self, index: usize) -> usize {
        self.doc.0.len().min(index + 1)
    }

    // Selectively repaints based on whether scroll or cursor position has
    // changed.  If only cursor position changed, then redraws the relevant
    // lines to minimize flickering.
    fn repaint(&mut self, cursor: usize, scroll: usize) -> Result<()> {
        if scroll != self.yscroll {
            // If the scroll position has changed, then we need to queue up
            // a full redraw of the whole screen.
            self.draw()?;
        } else if cursor != self.ycursor {
            // Otherwise, we only need to handle the lines near the cursor
            let mut out = std::io::stdout();

            for i in &[cursor, self.ycursor] {
                let sy = (*i - self.yscroll).try_into().unwrap();
                queue!(&mut out,
                    cursor::MoveTo(0, sy),
                    Clear(ClearType::CurrentLine),
                )?;
                self.draw_line(&mut out, *i)?;
            }
            out.flush()?;
        }
        Ok(())
    }

    fn down(&mut self) -> Result<()> {
        let prev_cursor = self.ycursor;
        let prev_scroll = self.yscroll;
        self.ycursor = self.increment_index(self.ycursor);

        // If we've scrolled off the bottom of the screen, then adjust the
        // scroll position as well
        if self.ycursor >= self.yscroll + self.size.1 as usize {
            self.yscroll = self.increment_index(self.yscroll);
        }
        self.repaint(prev_cursor, prev_scroll)
    }

    fn decrement_index(&self, index: usize) -> usize {
        index.saturating_sub(1)
    }

    fn up(&mut self) -> Result<()> {
        let prev_cursor = self.ycursor;
        let prev_scroll = self.yscroll;
        self.ycursor = self.decrement_index(self.ycursor);
        if self.ycursor < self.yscroll {
            self.yscroll = self.decrement_index(self.yscroll);
        }
        self.repaint(prev_cursor, prev_scroll)
    }
}

impl View {
    fn event(&mut self, evt: Event, view: &mut WrappedView) -> Result<bool> {
        match evt {
            Event::Key(event) => {
                match event.code {
                    KeyCode::Char('q') => { return Ok(false); }
                    KeyCode::Char('j') => view.down()?,
                    KeyCode::Char('k') => view.up()?,
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
                    MouseEvent::ScrollUp(..) => view.up()?,
                    MouseEvent::ScrollDown(..) => view.down()?,
                    _ => (),
                }
            },
            Event::Resize(w, h) => {
                view.resize((w, h))?;
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
