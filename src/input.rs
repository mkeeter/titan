use anyhow::Result;
use crossterm::{
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent},
};

struct Input(String);

impl Input {
    fn run(&mut self) -> Result<String> {
        execute!(std::io::stdout(),
            cursor::Show,
        )?;
        loop {
            let evt = read()?;
            if let Some(out) = self.event(evt)? {
                return Ok(out);
            }
        }
    }

    fn event(&mut self, evt: Event) -> Result<String> {
        match evt {
            Event::Key(event) => return self.key(event),
            _ => (),
        }
    }

    fn key(&mut self, k: KeyEvent) -> Result<String> {
        let sigint = k.code == KeyCode::Char('c') &&
                     k.modifiers == KeyModifiers::CONTROL;

        // Handle command editing if it is present
        if sigint {
            // Abort
        } else {
            match k.code {
                // On return, try to execute whatever is in the buffer
                KeyCode::Enter => {
                    self.0.take()
                },
                KeyCode::Backspace => { self.0.pop(); },
                KeyCode::Char(r) => { self.0.push(r); },
                _ => (),
            }
        }
        None
    }
}
