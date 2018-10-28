#[derive(Clone, Serialize, Deserialize)]
pub enum Face {
    Default,
    Match,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TextFragment {
    pub text: String,
    pub face: Face,
}

pub mod notification {
    /// Sent to to the client when connection is complete.
    pub mod info {
        use remote::jsonrpc::Notification;

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub session: String,
        }

        pub fn new(session: &str) -> Notification {
            let params = Params {
                session: session.to_string(),
            };
            Notification::new("info".to_string(), params).expect("new 'info' notification")
        }
    }

    pub mod view {
        use std::collections::HashMap;

        use editor::{Buffer, View, ViewItem};
        use remote::jsonrpc::Notification;

        #[derive(Clone, Serialize, Deserialize)]
        pub struct ParamsHeader {
            pub buffer: String,
            pub start: usize,
            pub end: usize,
        }

        #[derive(Clone, Serialize, Deserialize)]
        pub struct ParamsLines {
            pub lines: Vec<String>,
            pub first_line_num: usize,
        }

        #[derive(Clone, Serialize, Deserialize)]
        #[serde(tag = "type")]
        pub enum ParamsItem {
            Header(ParamsHeader),
            Lines(ParamsLines),
        }

        pub type Params = Vec<ParamsItem>;

        pub fn new(view: &View, buffers: &HashMap<String, Buffer>) -> Notification {
            let params: Params = view
                .as_vec()
                .iter()
                .map(|item| match item {
                    ViewItem::Header((buffer, focus)) => {
                        use editor::view::Focus;
                        match focus {
                            Focus::Range(range) => ParamsItem::Header(ParamsHeader {
                                buffer: buffer.to_string(),
                                start: range.start + 1,
                                end: range.end,
                            }),
                            Focus::Whole => {
                                let b = &buffers[&buffer.to_string()];
                                ParamsItem::Header(ParamsHeader {
                                    buffer: buffer.to_string(),
                                    start: 1,
                                    end: b.line_count(),
                                })
                            }
                        }
                    }
                    ViewItem::Lens(lens) => {
                        let buffer = &buffers[&lens.buffer];
                        ParamsItem::Lines(ParamsLines {
                            lines: buffer.lines(lens.focus.clone()).to_vec(),
                            first_line_num: lens.focus.start() + 1,
                        })
                    }
                }).collect();
            Notification::new("view".to_string(), params).expect("new 'init' notification")
        }
    }
}

pub mod request {
    pub mod command_list {
        use std::collections::BTreeMap;

        pub type Params = ();

        pub type Result = BTreeMap<String, String>;
    }

    pub mod buffer_select {
        use remote::jsonrpc::{Id, Request};

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub buffer: String,
        }

        pub fn new(id: Id, buffer: &str) -> Request {
            let params = Params {
                buffer: buffer.to_string(),
            };
            Request::new(id, "buffer-select".to_string(), params)
                .expect("new 'buffer-select' request")
        }
    }

    pub mod edit {
        use remote::jsonrpc::{Id, Request};

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub file: String,
            pub path: Option<String>,
        }

        pub fn new(id: Id, file: &str) -> Request {
            let params = Params {
                file: file.to_string(),
                path: None,
            };
            Request::new(id, "edit".to_string(), params).unwrap()
        }

        pub type Result = ();
    }

    pub mod view {
        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub view_id: String,
        }

        pub type Result = ();
    }

    pub mod menu {
        use protocol::TextFragment;
        use remote::jsonrpc::{Id, Request};

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub kind: String,
            pub search: String,
        }

        pub fn new(id: Id, kind: &str, search: &str) -> Request {
            let params = Params {
                kind: kind.to_string(),
                search: search.to_string(),
            };
            Request::new(id, "menu".to_string(), params).unwrap()
        }

        #[derive(Serialize, Deserialize)]
        pub struct Entry {
            pub text: String,
            pub fragments: Vec<TextFragment>,
        }

        #[derive(Serialize, Deserialize)]
        pub struct Result {
            pub kind: String,
            pub title: String,
            pub search: String,
            pub entries: Vec<Entry>,
        }
    }

    pub mod menu_select {
        use remote::jsonrpc::{Id, Request};

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub kind: String,
            pub choice: String,
        }

        pub fn new(id: Id, kind: &str, choice: &str) -> Request {
            let params = Params {
                kind: kind.to_string(),
                choice: choice.to_string(),
            };
            Request::new(id, "menu-select".to_string(), params).unwrap()
        }

        pub type Result = ();
    }
}
