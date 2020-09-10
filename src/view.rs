use std::convert::TryInto;
use std::io::{Write};

use crate::document::{Document, WrappedDocument, WrappedLine};
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

        WrappedView { doc, source, ycursor, yscroll,
            size: (tw, th),
            needs_redraw: true}
    }

    // Draws a line at the given index, starting at screen y pos sy
    fn draw_line<'a, W: Write>(&self, out: &mut W, line: &WrappedLine<'a>, highlight: bool, sy: u16)
            -> Result<()>
    {
        use Line_::*;
        let c = ContentStyle::new();

        // We trust that the line-wrapping has wrapped things like quotes and
        // links so that there's room for their prefixes here.
        let (v, q, first, later, c) = match line {
            Text(t) => (t.0, t.1, "", "", c),
            NamedLink { name, .. } => (name.0, name.1, "→ ", "  ", c.foreground(Color::Magenta)),
            H1(t) => (t.0, t.1, "# ", "  ", c.foreground(Color::DarkRed)),
            H2(t) => (t.0, t.1, "## ", "   ", c.foreground(Color::DarkYellow)),
            H3(t) => (t.0, t.1, "### ", "    ", c.foreground(Color::DarkCyan)),
            List(t) => (t.0, t.1, "• ", "  ", c),
            Quote(t) => (t.0, t.1, "> ", "> ", c.foreground(Color::White)),

            // TODO: handle overly long Pre and BareLink lines
            BareLink(url) => (*url, true, "→ ", "  ", c.foreground(Color::Magenta)),
            Pre { text, .. } => (text.0, text.1, "", "", c.foreground(Color::Red)),
        };
        let prefix = if q {
            first
        } else {
            later
        };

        if highlight {
            let c = c.background(Color::Black);
            let fill = " ".repeat((self.size.0 + 1).into());
            queue!(out,
                cursor::MoveTo(0, sy),
                PrintStyledContent(style(fill).on(Color::Black)),
                cursor::MoveTo(2, sy),
                PrintStyledContent(style(prefix).on(Color::Black)),
                PrintStyledContent(c.apply(v)))?;
        } else {
            queue!(out,
                cursor::MoveTo(2, sy),
                Print(prefix),
                PrintStyledContent(c.apply(v)),
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

        use std::iter::{repeat, once};
        for (sy, (line, active)) in (0..self.size.1)
            .zip(self.doc.0[self.yscroll..].iter()
                .zip(repeat(false)
                    .take(self.ycursor - self.yscroll)
                    .chain(once(true))
                    .chain(repeat(false)))) {
            self.draw_line(&mut out, line, active, sy.try_into().unwrap())?;
        }

        out.flush()?;
        Ok(())
    }

    // Safely increments a block/line index
    fn increment_index(&self, index: usize) -> usize {
        self.doc.0.len().min(index + 1)
    }

    fn down(&mut self) {
        let prev = (self.ycursor, self.yscroll);
        self.ycursor = self.increment_index(self.ycursor);

        // If we've scrolled off the bottom of the screen, then adjust the
        // scroll position as well
        if self.ycursor >= self.yscroll + self.size.1 as usize {
            self.yscroll = self.increment_index(self.yscroll);
        }
        self.needs_redraw = prev != (self.ycursor, self.yscroll);
    }

    fn decrement_index(&self, index: usize) -> usize {
        index.saturating_sub(1)
    }

    fn up(&mut self) {
        let prev = (self.ycursor, self.yscroll);
        self.ycursor = self.decrement_index(self.ycursor);
        if self.ycursor < self.yscroll {
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
                                         view.yscroll, view.ycursor);
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
