use std::convert::TryInto;
use std::io::{Write};

use crate::document::{Document, WrappedDocument};
use crate::input::Input;
use crate::protocol::{ResponseHeader, Line_};
use crate::command::Command;

use anyhow::Result;

use crossterm::{
    cursor,
    execute,
    terminal,
    event,
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent},
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

    has_cmd_error: bool,
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

        // Add two characters of padding on either side
        let tw = size.0 - 4;
        let doc = source.word_wrap((size.0 - 4).into());

        // Add a status and command bar at the bottom
        let th = size.1 - 2;

        let v = View { doc, source,
            ycursor: 0,
            yscroll: 0,
            size: (tw, th),
            has_cmd_error: false,
        };
        terminal::enable_raw_mode()
            .expect("Could not enable raw mode");
        execute!(std::io::stdout(), cursor::Hide, event::EnableMouseCapture)
            .expect("Could not hide cursor");
        v.draw();
        v
    }

    fn resize(&mut self, size: (u16, u16)) {
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

    fn draw_line<W: Write>(&self, out: &mut W, i: usize) {
        // We trust that the line-wrapping has wrapped things like quotes and
        // links so that there's room for their prefixes here.
        let p = Self::prefix;

        use Line_::*;
        let c = ContentStyle::new();
        let ((text, prefix), c) = match self.doc.0[i] {
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

    pub fn run(&mut self) -> Result<Command> {
        loop {
            let evt = read().expect("Could not read event");
            if let Some(cmd) = self.event(evt) {
                return cmd;
            }
        }
    }

    pub fn set_cmd_error(&mut self, err: &str) {
        let mut out = std::io::stdout();
        execute!(&mut out,
            cursor::MoveTo(0, self.size.1 + 1),
            Clear(ClearType::CurrentLine),
            PrintStyledContent(style(err).with(Color::DarkRed)),
        ).expect("Failed to queue cmd error");
        self.has_cmd_error = true;
    }

    fn clear_cmd(&mut self) {
        let mut out = std::io::stdout();
        execute!(&mut out,
            cursor::MoveTo(0, self.size.1 + 1),
            Clear(ClearType::CurrentLine),
        ).expect("Failed to queue cmd clear");
        self.has_cmd_error = false;
    }

    fn key(&mut self, k: KeyEvent) -> Option<Result<Command>> {
        // Exit on Ctrl-C, even though we don't get a true SIGINT
        if k.code == KeyCode::Char('c') &&
           k.modifiers == KeyModifiers::CONTROL
        {
            return Some(Ok(Command::Exit));
        }

        // Clear the command error pane on any keypress
        if self.has_cmd_error {
            self.clear_cmd();
        }


        // TODO: search mode with '/'
        // TODO: multiple up/down commands, e.g. 10j

        match k.code {
            KeyCode::Char('j') => { self.down(); None }
            KeyCode::Char('k') => { self.up(); None }
            KeyCode::Char(':') => {
                execute!(&mut std::io::stdout(),
                    cursor::MoveTo(0, self.size.1 + 1),
                    Print(":"),
                ).expect("Could not start drawing command line");
                if let Some(cmd) = Input::new().run() {
                    Some(Command::parse(cmd))
                } else {
                    self.clear_cmd();
                    None
                }
            },
            KeyCode::Enter => {
                match self.doc.0[self.ycursor] {
                    Line_::NamedLink { url, .. } |
                    Line_::BareLink(url) =>
                        Some(Ok(Command::TryLoad(url.to_string()))),
                    _ => None
                }
            },
            _ => None,
        }
    }

    fn event(&mut self, evt: Event) -> Option<Result<Command>> {
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
