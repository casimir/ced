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
        use jsonrpc::Notification;

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

    pub mod menu {
        use jsonrpc::Notification;
        use protocol::TextFragment;

        #[derive(Serialize, Deserialize)]
        pub struct Entry {
            pub value: String,
            pub fragments: Vec<TextFragment>,
            pub description: Option<String>,
        }

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub command: String,
            pub title: String,
            pub search: String,
            pub entries: Vec<Entry>,
        }

        pub fn new<P>(params: P) -> Notification
        where
            P: Into<Params>,
        {
            Notification::new("menu".to_string(), params.into()).unwrap()
        }
    }

    pub mod view {
        use jsonrpc::Notification;

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

        pub fn new<P>(params: P) -> Notification
        where
            P: Into<Params>,
        {
            Notification::new("view".to_string(), params.into()).expect("new 'init' notification")
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
        use jsonrpc::{Id, Request};

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
        use jsonrpc::{Id, Request};

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
        use jsonrpc::{Id, Request};

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub command: String,
            pub search: String,
        }

        pub fn new(id: Id, command: &str, search: &str) -> Request {
            let params = Params {
                command: command.to_string(),
                search: search.to_string(),
            };
            Request::new(id, "menu".to_string(), params).unwrap()
        }

        pub type Result = ();
    }

    pub mod menu_select {
        use jsonrpc::{Id, Request};

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub command: String,
            pub choice: String,
        }

        pub fn new(id: Id, command: &str, choice: &str) -> Request {
            let params = Params {
                command: command.to_string(),
                choice: choice.to_string(),
            };
            Request::new(id, "menu-select".to_string(), params).unwrap()
        }

        pub type Result = ();
    }
}
