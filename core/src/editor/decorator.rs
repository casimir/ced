use termion;

use remote::protocol::Markup;

pub struct Decorator(Markup);

impl Decorator {
    pub fn new(style: Markup) -> Decorator {
        Decorator(style)
    }

    pub fn em(&self, text: &str) -> String {
        match self.0 {
            Markup::None => text.to_owned(),
            Markup::Term => format!(
                "{}{}{}",
                termion::style::Underline,
                text,
                termion::style::NoUnderline
            ),
        }
    }
}
