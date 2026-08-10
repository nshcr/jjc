use std::collections::VecDeque;
use std::io;
use std::sync::Mutex;
use std::sync::OnceLock;

use crossterm::event;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

static SCRIPT: OnceLock<Mutex<Option<VecDeque<KeyEvent>>>> = OnceLock::new();

pub fn scripted() -> bool {
    std::env::var_os("JJC_KEYS").is_some()
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AppEvent {
    Key(KeyEvent),
    Resize,
}

pub fn read_event() -> io::Result<AppEvent> {
    if scripted() {
        let script = SCRIPT.get_or_init(|| Mutex::new(parse_env_script()));
        let mut script = script.lock().unwrap();
        let Some(events) = script.as_mut() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "JJC_KEYS is not valid",
            ));
        };
        return events.pop_front().map(AppEvent::Key).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "JJC_KEYS ran out of input")
        });
    }

    loop {
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                return Ok(AppEvent::Key(key));
            }
            Event::Resize(_, _) => return Ok(AppEvent::Resize),
            _ => {}
        }
    }
}

fn parse_env_script() -> Option<VecDeque<KeyEvent>> {
    parse_script(&std::env::var("JJC_KEYS").ok()?).ok()
}

fn parse_script(script: &str) -> Result<VecDeque<KeyEvent>, String> {
    let mut events = VecDeque::new();
    let mut chars = script.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut token = String::new();
            for c in chars.by_ref() {
                if c == '>' {
                    break;
                }
                token.push(c);
            }
            events.push_back(key_token(&token)?);
        } else {
            events.push_back(key(KeyCode::Char(c)));
        }
    }
    Ok(events)
}

fn key_token(token: &str) -> Result<KeyEvent, String> {
    match token {
        "Esc" => Ok(key(KeyCode::Esc)),
        "Enter" => Ok(key(KeyCode::Enter)),
        "Space" => Ok(key(KeyCode::Char(' '))),
        "Backspace" => Ok(key(KeyCode::Backspace)),
        "Delete" => Ok(key(KeyCode::Delete)),
        "Left" => Ok(key(KeyCode::Left)),
        "Right" => Ok(key(KeyCode::Right)),
        "Up" => Ok(key(KeyCode::Up)),
        "Down" => Ok(key(KeyCode::Down)),
        "PageUp" => Ok(key(KeyCode::PageUp)),
        "PageDown" => Ok(key(KeyCode::PageDown)),
        "F1" => Ok(key(KeyCode::F(1))),
        "C-r" => Ok(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
        "C-h" => Ok(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL)),
        "C-s" => Ok(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
        "C-c" => Ok(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        "C-w" => Ok(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)),
        "C-u" => Ok(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)),
        _ => Err(format!("unknown key token: {token}")),
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_keys() {
        let keys = parse_script("iHi<Esc>:wq<Enter><C-r><C-s><C-c><F1><PageDown><PageUp>").unwrap();
        assert_eq!(keys.len(), 14);
    }
}
