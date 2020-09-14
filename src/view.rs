use std::convert::TryInto;
use std::io::{Write};

use crate::document::{Document, WrappedDocument};
use crate::protocol::{ResponseHeader, Line_};

use anyhow::{Result};
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

    cmd: Option<String>,
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
    pub fn new<'a>(source: &'a Document) -> Result<View<'a>> {
        let size = terminal::size()?;

        // Add two characters of padding on either side
        let tw = size.0 - 4;
        let doc = source.word_wrap((size.0 - 4).into());

        // Add a status and command bar at the bottom
        let th = size.1 - 2;

        Ok(View { doc, source,
            ycursor: 0,
            yscroll: 0,
            size: (tw, th),
            cmd: None,
            has_cmd_error: false,
        })
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

    pub fn run(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(std::io::stdout(), cursor::Hide, event::EnableMouseCapture)?;
        self.draw()?;

        loop {
            let evt = read()?;
            if !self.event(evt)? {
                return Ok(());
            }
        }
    }

    fn execute_cmd(&mut self) -> Result<bool> {
        let cmd = self.cmd.take().expect("Can't execute empty cmd");
        if cmd == "q" {
            Ok(false)
        } else {
            let mut out = std::io::stdout();
            queue!(&mut out,
                cursor::MoveTo(0, self.size.1 + 1),
                Clear(ClearType::CurrentLine),
                PrintStyledContent(
                    style(format!("Unknown command: {}", cmd))
                        .with(Color::DarkRed)),
            )?;
            out.flush()?;
            self.has_cmd_error = true;
            Ok(true)
        }
    }

    fn repaint_cmd(&mut self) -> Result<()> {
        let mut out = std::io::stdout();
        queue!(&mut out,
            cursor::MoveTo(0, self.size.1 + 1),
            Clear(ClearType::CurrentLine),
        )?;

        if let Some(c) = &self.cmd {
            queue!(&mut out,
                Print(":"),
                Print(c),
            )?;
        }
        out.flush()?;
        Ok(())
    }

    fn key(&mut self, k: KeyEvent) -> Result<bool> {
        if self.has_cmd_error {
            self.repaint_cmd()?;
            self.has_cmd_error = false;
        }

        let sigint = k.code == KeyCode::Char('c') &&
                     k.modifiers == KeyModifiers::CONTROL;

        if let Some(c) = &mut self.cmd {
            if sigint {
                self.cmd = None;
            } else {
                match k.code {
                    KeyCode::Enter => return self.execute_cmd(),
                    KeyCode::Backspace => { c.pop(); },
                    KeyCode::Char(r) => { c.push(r); },
                    _ => (),
                }
            }
            self.repaint_cmd()?;
            return Ok(true);
        }

        if sigint {
            return Ok(false);
        }

        match k.code {
            KeyCode::Char('j') => self.down()?,
            KeyCode::Char('k') => self.up()?,
            KeyCode::Char(':') => {
                self.cmd = Some(String::new());
                self.repaint_cmd()?;
                return Ok(true);
            },
            _ => (),
        }
        Ok(true)
    }

    fn event(&mut self, evt: Event) -> Result<bool> {
        match evt {
            Event::Key(event) => return self.key(event),
            Event::Mouse(event) => {
                match event {
                    MouseEvent::ScrollUp(..) => self.up()?,
                    MouseEvent::ScrollDown(..) => self.down()?,
                    _ => (),
                }
            },
            Event::Resize(w, h) => {
                self.resize((w, h))?;
            },
        }
        Ok(true)
    }
}
