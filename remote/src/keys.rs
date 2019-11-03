use std::fmt;
use std::str::FromStr;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Key {
    Char(char),
    Escape,
}

impl Default for Key {
    fn default() -> Self {
        Key::Char('\0')
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Key::*;
        match self {
            Char(c) => write!(f, "{}", c),
            Escape => f.write_str("esc"),
        }
    }
}

#[derive(Debug)]
pub struct ParseKeyError {
    raw_source: String,
}

impl FromStr for Key {
    type Err = ParseKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.chars().count() == 1 {
            return Ok(Key::Char(s.chars().next().unwrap()));
        }

        match s {
            "esc" => Ok(Key::Escape),
            _ => Err(ParseKeyError {
                raw_source: s.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: Key,
}

impl From<Key> for KeyEvent {
    fn from(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            ..Default::default()
        }
    }
}

impl From<&str> for KeyEvent {
    fn from(s: &str) -> KeyEvent {
        let mut event = KeyEvent::default();
        let mut cursor = 0;
        if s[cursor..].starts_with("c-") {
            event.ctrl = true;
            cursor += 2;
        }
        if s[cursor..].starts_with("a-") {
            event.alt = true;
            cursor += 2;
        }
        if s[cursor..].starts_with("s-") {
            event.shift = true;
            cursor += 2;
        }
        event.key = s
            .chars()
            .skip(cursor)
            .collect::<String>()
            .parse()
            .expect("extract key value");
        event
    }
}

impl From<char> for KeyEvent {
    fn from(c: char) -> KeyEvent {
        KeyEvent {
            key: Key::Char(c.to_ascii_lowercase()),
            shift: c.is_ascii_uppercase(),
            ..Default::default()
        }
    }
}

impl fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();
        if self.ctrl {
            s += "c-";
        }
        if self.alt {
            s += "a-";
        }
        if self.shift {
            s += "s-";
        }
        write!(f, "{}{}", s, self.key)
    }
}

impl From<KeyEvent> for Vec<KeyEvent> {
    fn from(event: KeyEvent) -> Vec<KeyEvent> {
        vec![event]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert() {
        assert_eq!(
            KeyEvent::from("a"),
            KeyEvent {
                key: Key::Char('a'),
                ..Default::default()
            }
        );
        assert_eq!(
            KeyEvent::from("c-a"),
            KeyEvent {
                ctrl: true,
                key: Key::Char('a'),
                ..Default::default()
            }
        );
        assert_eq!(
            KeyEvent::from("c-s-a"),
            KeyEvent {
                ctrl: true,
                shift: true,
                key: Key::Char('a'),
                ..Default::default()
            }
        );
        assert_eq!(
            KeyEvent::from('c'),
            KeyEvent {
                key: Key::Char('c'),
                ..Default::default()
            }
        );

        {
            let k = KeyEvent {
                alt: true,
                key: Key::Char('é'),
                ..Default::default()
            };
            assert_eq!(k, KeyEvent::from(k.to_string().as_str()));
        }
        {
            let k = KeyEvent {
                ctrl: true,
                alt: true,
                key: Key::Char(','),
                ..Default::default()
            };
            assert_eq!(k, KeyEvent::from(k.to_string().as_str()));
        }
    }
}
