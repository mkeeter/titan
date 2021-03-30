use std::convert::TryInto;
use std::io::{Write};

use silo::document::Document;
use silo::protocol::Line;

use crate::wrapped::WrappedDocument;
use crate::command::Command;

use anyhow::Result;

use crossterm::{
    cursor,
    event,
    execute,
    terminal,
    event::{Event, KeyCode, KeyEvent, MouseEvent},
    terminal::{Clear, ClearType},
    style::{style, Color, ContentStyle, Print, PrintStyledContent},
    queue,
};

pub struct View<'a> {
    source: &'a Document<'a>,
    doc: WrappedDocument<'a>,

    size: (u16, u16), // width, height

    yscroll: usize, // Y scoll position in the doc
    ycursor: usize, // Y cursor position in the doc
}

impl Drop for View<'_> {
    fn drop(&mut self) {
        execute!(std::io::stdout(),
            cursor::Show,
            event::DisableMouseCapture,
            terminal::Clear(ClearType::All),
        ).expect("Could not renable cursor");
        terminal::disable_raw_mode()
            .expect("Could not disable raw mode");
    }
}

impl View<'_> {
    pub fn new<'a>(source: &'a Document) -> View<'a> {
        let size = terminal::size()
            .expect("Could not get terminal size");

        let doc = crate::wrapped::dummy_wrap(source);

        let mut v = View { doc, source,
            ycursor: 0,
            yscroll: 0,
            size: (0, 0),
        };
        terminal::enable_raw_mode()
            .expect("Could not enable raw mode");
        execute!(std::io::stdout(), cursor::Hide, event::EnableMouseCapture)
            .expect("Could not hide cursor");
        v.resize(size);
        v.draw();
        v
    }

    fn resize(&mut self, size: (u16, u16)) {
        // Attempt to maintain roughly the same scroll and cursor position
        // after resizing is complete
        let yscroll_frac = self.yscroll as f32 / self.doc.0.len() as f32;
        let ycursor_frac = self.ycursor as f32 / self.doc.0.len() as f32;

        self.doc = crate::wrapped::word_wrap(self.source, (size.0 - 4).into());

        // Add two characters of padding on either side, and a status
        // and command bar at the bottom
        // Add a status and command bar at the bottom
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

    fn draw_line<W: Write>(&self, out: &mut W, i: usize) {
        // We trust that the line-wrapping has wrapped things like quotes and
        // links so that there's room for their prefixes here.

        use Line::*;
        let c = ContentStyle::new();
        let (line, first) = self.doc.0[i];

        // Prefix selector function
        let p = |a, b| if first { a } else { b };

        let (text, prefix, c) = match line {
            Text(t) => (t, "", c),
            H1(t) => (t, p("# ", "  "), c.foreground(Color::DarkRed)),
            H2(t) => (t, p("## ", "   "), c.foreground(Color::DarkYellow)),
            H3(t) => (t, p("### ", "    "), c.foreground(Color::DarkCyan)),
            List(t) => (t, p("• ", "  "), c),
            Quote(t) => (t, "> ", c.foreground(Color::White)),
            NamedLink { name, .. } => (name, p("→ ", "  "),
                                       c.foreground(Color::Magenta)),

            // TODO: handle overly long Pre and BareLink lines
            BareLink(url) => (url, "→ ", c.foreground(Color::Magenta)),
            Pre { text, .. } => (text, "", c.foreground(Color::Red)),
        };

        let sy = (i - self.yscroll).try_into().unwrap();
        assert!(sy < self.size.1);

        if i == self.ycursor {
            let c = c.background(Color::Black);
            let fill = " ".repeat((self.size.0 + 1).into());
            queue!(out,
                cursor::MoveTo(0, sy),
                PrintStyledContent(style(fill).on(Color::Black)),
                cursor::MoveTo(2, sy),
                PrintStyledContent(style(prefix).on(Color::Black)),
                PrintStyledContent(c.apply(text)),
            )
        } else {
            queue!(out,
                cursor::MoveTo(2, sy),
                Print(prefix),
                PrintStyledContent(c.apply(text)),
            )
        }.expect("Could not queue line");
    }

    fn draw(&self) {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();

        queue!(out,
            cursor::MoveTo(self.size.0 + 4, self.size.1 - 1),
            Clear(ClearType::FromCursorUp),
        ).expect("Could not queue clear");

        for i in (0..self.size.1)
            .map(|i| i as usize + self.yscroll)
            .take_while(|i| *i < self.doc.0.len())
        {
            self.draw_line(&mut out, i);
        }

        out.flush().expect("Could not flush stdout");
    }

    // Safely increments a line index
    fn increment_index(&self, index: usize) -> usize {
        (index + 1).min(self.doc.0.len() - 1)
    }

    // Selectively repaints based on whether scroll or cursor position has
    // changed.  If only cursor position changed, then redraws the relevant
    // lines to minimize flickering.
    fn repaint(&mut self, cursor: usize, scroll: usize) {
        if scroll != self.yscroll {
            // If the scroll position has changed, then we need to queue up
            // a full redraw of the whole screen.
            self.draw();
        } else if cursor != self.ycursor {
            // Otherwise, we only need to handle the lines near the cursor
            let mut out = std::io::stdout();

            for i in &[cursor, self.ycursor] {
                let sy = (*i - self.yscroll).try_into().unwrap();
                queue!(&mut out,
                    cursor::MoveTo(0, sy),
                    Clear(ClearType::CurrentLine),
                ).expect("Could not queue cursor move");
                self.draw_line(&mut out, *i);
            }
            out.flush().expect("Failed to flush stdout");
        }
    }

    fn down(&mut self) {
        let prev_cursor = self.ycursor;
        let prev_scroll = self.yscroll;
        self.ycursor = self.increment_index(self.ycursor);

        // If we've scrolled off the bottom of the screen, then adjust the
        // scroll position as well
        if self.ycursor >= self.yscroll + self.size.1 as usize {
            self.yscroll = self.increment_index(self.yscroll);
        }
        self.repaint(prev_cursor, prev_scroll);
    }

    fn decrement_index(&self, index: usize) -> usize {
        index.saturating_sub(1)
    }

    fn up(&mut self) {
        let prev_cursor = self.ycursor;
        let prev_scroll = self.yscroll;
        self.ycursor = self.decrement_index(self.ycursor);
        if self.ycursor < self.yscroll {
            self.yscroll = self.decrement_index(self.yscroll);
        }
        self.repaint(prev_cursor, prev_scroll)
    }

    fn key(&mut self, k: KeyEvent) -> Option<Result<Command>> {
        match k.code {
            KeyCode::Char('j') => { self.down(); None }
            KeyCode::Char('k') => { self.up(); None }
            KeyCode::Enter => {
                match self.doc.0[self.ycursor].0 {
                    Line::NamedLink { url, .. } |
                    Line::BareLink(url) =>
                        Some(Ok(Command::TryLoad(url.to_string()))),
                    _ => None
                }
            },
            _ => None,
        }
    }

    pub fn event(&mut self, evt: Event) -> Option<Result<Command>> {
        match evt {
            Event::Key(event) => self.key(event),
            Event::Mouse(event) => {
                match event {
                    MouseEvent::ScrollUp(..) => self.up(),
                    MouseEvent::ScrollDown(..) => self.down(),
                    _ => (),
                };
                None
            },
            Event::Resize(w, h) => {
                self.resize((w, h));
                None
            },
        }
    }
}
