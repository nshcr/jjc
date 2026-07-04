use std::collections::VecDeque;
use std::io;
use std::sync::Mutex;
use std::sync::OnceLock;

use crossterm::event;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

static SCRIPT: OnceLock<Mutex<Option<VecDeque<KeyEvent>>>> = OnceLock::new();

pub fn scripted() -> bool {
    std::env::var_os("JJC_KEYS").is_some()
}

pub fn read_key() -> io::Result<KeyEvent> {
    if scripted() {
        let script = SCRIPT.get_or_init(|| Mutex::new(parse_env_script()));
        let mut script = script.lock().unwrap();
        let Some(events) = script.as_mut() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "JJC_KEYS is not valid",
            ));
        };
        return events.pop_front().ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "JJC_KEYS ran out of input")
        });
    }

    loop {
        if let Event::Key(key) = event::read()? {
            return Ok(key);
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
        "C-r" => Ok(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
        "C-h" => Ok(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL)),
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
        let keys = parse_script("iHi<Esc>:wq<Enter><C-r>").unwrap();
        assert_eq!(keys.len(), 9);
    }
}
