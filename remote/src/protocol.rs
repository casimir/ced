use std::fmt;

pub use crate::keys::Key;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Face {
    Default,
    Error,
    Match,
    Prompt,
    Selection,
}

// used in ffi to convert enum value to string
impl fmt::Display for Face {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Default for Face {
    fn default() -> Face {
        Face::Default
    }
}

pub type Text = ornament::Text<Face>;
pub type TextFragment = ornament::TextFragment<Face>;

pub mod notifications {
    use crate::jsonrpc::Notification as JNotification;
    use crate::protocol::Text;

    pub trait Notification {
        const METHOD: &'static str;
        type Params: serde::Serialize;

        fn new<P>(p: P) -> JNotification
        where
            P: Into<Self::Params>,
        {
            let params: Self::Params = p.into();
            JNotification::new(Self::METHOD, params)
                .expect(&format!("new {} notification", Self::METHOD))
        }

        fn new_noarg() -> JNotification {
            JNotification::new(Self::METHOD, ())
                .expect(&format!("new {} notification", Self::METHOD))
        }
    }

    macro_rules! notification {
        ($name:ident, $method:expr, $params:ty) => {
            pub struct $name;
            impl Notification for $name {
                const METHOD: &'static str = $method;
                type Params = $params;
            }
        };
    }

    notification!(Echo, "echo", Text);
    notification!(Info, "info", InfoParams);
    notification!(Menu, "menu", MenuParams);
    notification!(Status, "status", StatusParams);
    notification!(View, "view", ViewParams);

    #[derive(Serialize, Deserialize)]
    pub struct InfoParams {
        pub client: String,
        pub session: String,
        pub cwd: String,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct MenuParamsEntry {
        pub value: String,
        pub text: Text,
        pub description: Option<String>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct MenuParams {
        pub command: String,
        pub title: String,
        pub search: String,
        pub entries: Vec<MenuParamsEntry>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct ViewParamsHeader {
        pub buffer: String,
        pub start: usize,
        pub end: usize,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct ViewParamsLines {
        pub lines: Vec<Text>,
        pub first_line_num: usize,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct StatusParamsItem {
        pub index: isize,
        pub text: Text,
    }

    pub type StatusParams = Vec<StatusParamsItem>;

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum ViewParamsItem {
        Header(ViewParamsHeader),
        Lines(ViewParamsLines),
    }

    pub type ViewParams = Vec<ViewParamsItem>;
}

pub mod requests {
    use super::Key;
    use crate::jsonrpc::{Id, Request as JRequest};

    pub trait Request {
        const METHOD: &'static str;
        type Params: serde::Serialize;
        type Result;

        fn new<P>(id: Id, p: P) -> JRequest
        where
            P: Into<Self::Params>,
        {
            let params: Self::Params = p.into();
            JRequest::new(id, Self::METHOD, params).expect(&format!("new {} request", Self::METHOD))
        }

        fn new_noarg(id: Id) -> JRequest {
            JRequest::new(id, Self::METHOD, ()).expect(&format!("new {} request", Self::METHOD))
        }
    }

    macro_rules! request {
        ($name:ident, $method:expr, $params:ty, $result:ty) => {
            pub struct $name;
            impl Request for $name {
                const METHOD: &'static str = $method;
                type Params = $params;
                type Result = $result;
            }
        };
    }

    request!(Quit, "quit", (), ());
    request!(Edit, "edit", EditParams, ());
    request!(View, "view", String, ());
    request!(ViewDelete, "view-delete", (), ());
    request!(ViewAdd, "view-add", String, ());
    request!(ViewRemove, "view-remove", String, ());
    request!(Menu, "menu", MenuParams, ());
    request!(MenuSelect, "menu-select", MenuSelectParams, ());
    request!(Keys, "keys", Vec<Key>, ());

    #[derive(Serialize, Deserialize)]
    pub struct EditParams {
        pub file: String,
        pub path: Option<String>,
        pub scratch: bool,
    }

    #[derive(Serialize, Deserialize)]
    pub struct MenuParams {
        pub command: String,
        pub search: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct MenuSelectParams {
        pub command: String,
        pub choice: String,
    }
}
