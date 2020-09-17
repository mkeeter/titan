use std::io::Write;

use crossterm::{
    cursor,
    execute,
    cursor::MoveLeft,
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Print},
};

pub struct Input(String);

impl Drop for Input {
    fn drop(&mut self) {
        execute!(std::io::stdout(),
            cursor::Hide,
        ).expect("Could not hide cursor");
    }
}

impl Input {
    pub fn new() -> Input {
        Input(String::new())
    }

    pub fn run(&mut self) -> Option<String> {
        execute!(std::io::stdout(),
            cursor::Show,
        ).expect("Failed to execute");
        loop {
            let evt = read().expect("Failed to read event");
            match evt {
                Event::Key(KeyEvent { code: KeyCode::Enter, .. }) => {
                    return Some(self.0.clone());
                },
                Event::Key(event) =>
                    if !self.key(event) {
                        return None;
                    }
                _ => continue,
            }
        }
    }

    fn key(&mut self, k: KeyEvent) -> bool {
        let sigint = k.code == KeyCode::Char('c') &&
                     k.modifiers == KeyModifiers::CONTROL;

        // Cancel on Ctrl-C or escape
        if sigint || k.code == KeyCode::Esc {
            return false;
        }

        // Otherwise, edit the buffer and redraw
        let mut out = std::io::stdout();
        match k.code {
            KeyCode::Backspace => {
                if !self.0.is_empty() {
                    self.0.pop();
                    execute!(&mut out,
                        MoveLeft(1),
                        Print(" "),
                        MoveLeft(1),
                    ).expect("Failed to execute");
                }
            },
            KeyCode::Char(r) => {
                self.0.push(r);
                execute!(&mut out,
                    Print(r),
                ).expect("Failed to execute");
            },
            _ => (),
        }
        true
    }
}
