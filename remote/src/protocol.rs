#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Face {
    Default,
    Match,
    Prompt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextFragment {
    pub text: String,
    pub face: Face,
}

pub mod notification {
    /// Sent to to the client when connection is complete.
    pub mod info {
        use crate::jsonrpc::Notification;

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub client: String,
            pub session: String,
            pub cwd: String,
        }

        pub fn new(client_id: usize, session: &str, cwd: &str) -> Notification {
            let params = Params {
                client: client_id.to_string(),
                session: session.to_string(),
                cwd: cwd.to_string(),
            };
            Notification::new("info".to_string(), params).expect("new 'info' notification")
        }
    }

    pub mod menu {
        use crate::jsonrpc::Notification;
        use crate::protocol::TextFragment;

        #[derive(Clone, Debug, Serialize, Deserialize)]
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
        use crate::jsonrpc::Notification;

        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct ParamsHeader {
            pub buffer: String,
            pub start: usize,
            pub end: usize,
        }

        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct ParamsLines {
            pub lines: Vec<String>,
            pub first_line_num: usize,
        }

        #[derive(Clone, Debug, Serialize, Deserialize)]
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

        use crate::jsonrpc::{Id, Request};

        pub type Params = ();

        pub type Result = BTreeMap<String, String>;

        pub fn new(id: Id) -> Request {
            Request::new(id, "command_list".to_string(), ()).expect("new command_list request")
        }
    }

    pub mod quit {
        use crate::jsonrpc::{Id, Request};

        pub type Params = ();

        pub type Result = ();

        pub fn new(id: Id) -> Request {
            Request::new(id, "quit".to_string(), ()).expect("new quit request")
        }
    }

    pub mod edit {
        use crate::jsonrpc::{Id, Request};

        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub file: String,
            pub path: Option<String>,
            pub scratch: bool,
        }

        pub fn new(id: Id, file: &str, scratch: bool) -> Request {
            let params = Params {
                file: file.to_string(),
                path: None,
                scratch,
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

    pub mod view_add {
        #[derive(Serialize, Deserialize)]
        pub struct Params {
            pub buffer: String,
        }

        pub type Result = ();
    }

    pub mod menu {
        use crate::jsonrpc::{Id, Request};

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
        use crate::jsonrpc::{Id, Request};

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
