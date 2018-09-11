use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use jsonrpc_lite;
use serde_json;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Id {
    Number(i64),
    String(String),
}

impl From<jsonrpc_lite::Id> for Id {
    fn from(id: jsonrpc_lite::Id) -> Id {
        use self::Id::*;
        match id {
            jsonrpc_lite::Id::Num(n) => Number(n),
            jsonrpc_lite::Id::Str(s) => String(s),
            jsonrpc_lite::Id::None(_) => unreachable!(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Object {
    inner: jsonrpc_lite::JsonRpc,
    pub id: Option<Id>,
}

impl Object {
    pub fn inner(&self) -> &jsonrpc_lite::JsonRpc {
        &self.inner
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json = serde_json::to_value(self.inner()).map_err(|_| fmt::Error)?;
        let payload = serde_json::to_string(&json).map_err(|_| fmt::Error)?;
        f.write_str(&payload)
    }
}

impl FromStr for Object {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        jsonrpc_lite::JsonRpc::parse(s).map(Object::from)
    }
}

impl From<jsonrpc_lite::JsonRpc> for Object {
    fn from(object: jsonrpc_lite::JsonRpc) -> Object {
        let id = object.get_id().map(|i| i.into());
        Object { inner: object, id }
    }
}

type Buffer = HashMap<String, String>;

pub mod notification {
    /// Sent to to the client when connection is complete.
    pub mod init {
        use jsonrpc_lite;

        use super::super::Buffer;

        pub struct Params {
            pub buffer_list: Vec<Buffer>,
            pub buffer_current: String,
        }

        impl From<jsonrpc_lite::Params> for Params {
            fn from(params: jsonrpc_lite::Params) -> Params {
                if let jsonrpc_lite::Params::Map(value) = params {
                    let mut buffer_list: Vec<Buffer> = Vec::new();
                    for buffer_value in value["buffer_list"].as_array().unwrap() {
                        let mut buffer = Buffer::new();
                        for (k, v) in buffer_value.as_object().unwrap() {
                            buffer.insert(k.to_owned(), v.as_str().unwrap().to_owned());
                        }
                        buffer_list.push(buffer);
                    }
                    Params {
                        buffer_list,
                        buffer_current: value["buffer_current"].as_str().unwrap().to_string(),
                    }
                } else {
                    panic!("invalid init params: {:?}", params)
                }
            }
        }
    }

    /// Sent to all clients when a buffer has changed (e.g. content modified).
    pub mod buffer_changed {
        use std::collections::HashMap;
        use std::ops::Deref;

        use jsonrpc_lite;
        use serde_json::Value;

        use super::super::Buffer;

        pub struct Params(Buffer);

        impl Deref for Params {
            type Target = Buffer;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl From<Value> for Params {
            fn from(value: Value) -> Params {
                let mut buffer: HashMap<String, String> = HashMap::new();
                for (k, v) in value.as_object().unwrap() {
                    buffer.insert(k.to_owned(), v.as_str().unwrap().to_owned());
                }
                Params(buffer)
            }
        }

        impl From<jsonrpc_lite::Params> for Params {
            fn from(params: jsonrpc_lite::Params) -> Params {
                if let jsonrpc_lite::Params::Map(value) = params {
                    let mut buffer: HashMap<String, String> = HashMap::new();
                    for (k, v) in &value {
                        buffer.insert(k.to_owned(), v.as_str().unwrap().to_owned());
                    }
                    Params(buffer)
                } else {
                    panic!("invalid init params: {:?}", params)
                }
            }
        }
    }
}

pub mod request {
    pub mod buffer_select {
        use std::ops::Deref;

        use jsonrpc_lite;
        use serde_json::Value;

        use super::super::Object;

        pub struct Params<'a>(pub &'a str);

        pub fn new(id: i64, params: &Params) -> Object {
            let values = vec![Value::from(params.0)];
            let inner = jsonrpc_lite::JsonRpc::request_with_params(id, "buffer-select", values);
            let id = Some(inner.get_id().unwrap().into());
            Object { inner, id }
        }

        pub struct Result(String);

        impl Deref for Result {
            type Target = String;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl From<Value> for Result {
            fn from(value: Value) -> Result {
                Result(value.as_str().unwrap().to_string())
            }
        }
    }

    pub mod edit {
        use std::ops::Deref;

        use jsonrpc_lite;
        use serde_json::Value;

        use super::super::Object;

        pub struct Params<'a>(pub Vec<&'a str>);

        pub fn new(id: i64, params: &Params) -> Object {
            let values = params.0.iter().map(|&e| e.into()).collect::<Vec<Value>>();
            let inner = jsonrpc_lite::JsonRpc::request_with_params(id, "edit", values);
            let id = Some(inner.get_id().unwrap().into());
            Object { inner, id }
        }

        pub struct Result(String);

        impl Deref for Result {
            type Target = String;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl From<Value> for Result {
            fn from(value: Value) -> Result {
                Result(value.as_str().unwrap().to_string())
            }
        }
    }
}
