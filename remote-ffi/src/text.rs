use std::os::raw::c_char;

use crate::cstring;
use crate::IndexedIterator;
use remote::protocol::{Text, TextFragment};

pub struct CedTextItem {
    pub text: *const c_char,
    pub face: *const c_char,
}

impl From<TextFragment> for CedTextItem {
    fn from(tf: TextFragment) -> CedTextItem {
        CedTextItem {
            text: cstring!(tf.text.as_str()),
            face: cstring!(tf.face.to_string()),
        }
    }
}

pub type CedTextIterator = IndexedIterator<TextFragment, CedTextItem>;

impl From<&Text> for CedTextIterator {
    fn from(text: &Text) -> CedTextIterator {
        CedTextIterator::from(
            &text
                .iter()
                .map(ToOwned::to_owned)
                .collect::<Vec<TextFragment>>(),
        )
    }
}
