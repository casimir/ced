use std::fmt;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub value: char,
}

impl From<&str> for Key {
    fn from(s: &str) -> Key {
        let mut key = Key::default();
        let mut cursor = 0;
        if s[cursor..].starts_with("c-") {
            key.ctrl = true;
            cursor += 2;
        }
        if s[cursor..].starts_with("a-") {
            key.alt = true;
            cursor += 2;
        }
        if s[cursor..].starts_with("s-") {
            key.shift = true;
            cursor += 2;
        }
        key.value = s.chars().nth(cursor).expect("extract key value");
        key
    }
}

impl From<char> for Key {
    fn from(c: char) -> Key {
        Key {
            value: c.to_lowercase().next().unwrap(),
            shift: c.is_uppercase(),
            ..Default::default()
        }
    }
}

impl fmt::Display for Key {
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
        s.push(self.value);
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert() {
        assert_eq!(
            Key::from("a"),
            Key {
                value: 'a',
                ..Default::default()
            }
        );
        assert_eq!(
            Key::from("c-a"),
            Key {
                ctrl: true,
                value: 'a',
                ..Default::default()
            }
        );
        assert_eq!(
            Key::from("c-s-a"),
            Key {
                ctrl: true,
                shift: true,
                value: 'a',
                ..Default::default()
            }
        );
        assert_eq!(
            Key::from('c'),
            Key {
                value: 'c',
                ..Default::default()
            }
        );

        {
            let k = Key {
                alt: true,
                value: 'é',
                ..Default::default()
            };
            assert_eq!(k, Key::from(k.to_string().as_str()));
        }
        {
            let k = Key {
                ctrl: true,
                alt: true,
                value: ',',
                ..Default::default()
            };
            assert_eq!(k, Key::from(k.to_string().as_str()));
        }
    }
}
