use std::fmt;
use std::str::FromStr;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{from_value, to_string, to_value, Value};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Number(i32),
    String(String),
    Null,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    jsonrpc: String,
    pub id: Id,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

impl Request {
    pub fn new<T, P>(id: Id, method: String, params: P) -> serde_json::Result<Request>
    where
        T: Serialize,
        P: Into<Option<T>>,
    {
        let serialized_params = match params.into() {
            Some(p) => Some(serde_json::to_value(p)?),
            None => None,
        };
        Ok(Request {
            jsonrpc: "2.0".to_string(),
            id,
            method,
            params: serialized_params,
        })
    }

    pub fn params<T>(&self) -> serde_json::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        if self.params.is_none() {
            return Ok(None);
        }
        from_value(self.params.clone().unwrap())
    }
}

impl FromStr for Request {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = to_string(&self).map_err(|_| fmt::Error)?;
        f.write_str(&payload)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

impl Notification {
    pub fn new<T, P>(method: String, params: P) -> serde_json::Result<Notification>
    where
        T: Serialize,
        P: Into<Option<T>>,
    {
        let serialized_params = match params.into() {
            Some(p) => Some(serde_json::to_value(p)?),
            None => None,
        };
        Ok(Notification {
            jsonrpc: "2.0".to_string(),
            method,
            params: serialized_params,
        })
    }

    pub fn params<T>(&self) -> serde_json::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        if self.params.is_none() {
            return Ok(None);
        }
        from_value(self.params.clone().unwrap())
    }
}

impl FromStr for Notification {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl fmt::Display for Notification {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = to_string(&self).map_err(|_| fmt::Error)?;
        f.write_str(&payload)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Error {
    code: i32,
    message: String,
    data: Option<Value>,
}

impl Error {
    pub fn new<T, D>(code: i32, message: String, data: D) -> serde_json::Result<Error>
    where
        T: Serialize,
        D: Into<Option<T>>,
    {
        let serialized_data = match data.into() {
            Some(d) => Some(serde_json::to_value(d)?),
            None => None,
        };
        Ok(Error {
            code,
            message,
            data: serialized_data,
        })
    }

    pub fn data<T>(&self) -> serde_json::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        if self.data.is_none() {
            return Ok(None);
        }
        from_value(self.data.clone().unwrap())
    }

    // TODO -32700   Parse error         Invalid JSON was received by the server.
    //                              An error occurred on the server while parsing the JSON text.
    // -32600   Invalid Request     The JSON sent is not a valid Request object.
    // -32601   Method not found    The method does not exist / is not available.
    // -32602   Invalid params 	    Invalid method parameter(s).
    // -32603   Internal error      Internal JSON-RPC error.

    pub fn invalid_request(source: &str) -> Error {
        Error::new(
            -32600,
            "The JSON sent is not a valid Request object.".to_string(),
            source,
        )
        .unwrap()
    }

    pub fn method_not_found(method: &str) -> Error {
        Error::new(
            -32601,
            "The method does not exist / is not available.".to_string(),
            method,
        )
        .unwrap()
    }

    pub fn invalid_params(reason: &str) -> Error {
        Error::new(-32602, "Invalid method parameter(s).".to_string(), reason).unwrap()
    }

    pub fn internal_error(details: &str) -> Error {
        Error::new(-32603, "Internal JSON-RPC error.".to_string(), details).unwrap()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = to_string(&self).map_err(|_| fmt::Error)?;
        f.write_str(&payload)
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    jsonrpc: String,
    pub id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Error>,
}

impl Response {
    pub fn new<T>(id: Id, result: Result<T, Error>) -> serde_json::Result<Response>
    where
        T: Serialize,
    {
        let mut resp = Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: None,
        };
        resp.set_result(result)?;
        Ok(resp)
    }

    pub fn result<T>(&self) -> serde_json::Result<Result<T, Error>>
    where
        T: DeserializeOwned,
    {
        Ok(match self.result {
            Some(ref result) => Ok(from_value(result.clone())?),
            None => Err(self.error.clone().unwrap()),
        })
    }

    pub fn set_result<T>(&mut self, result: Result<T, Error>) -> serde_json::Result<()>
    where
        T: Serialize,
    {
        match result {
            Ok(res) => {
                self.result = Some(to_value(res)?);
                self.error = None;
            }
            Err(err) => {
                self.result = None;
                self.error = Some(err);
            }
        }
        Ok(())
    }

    pub fn success<T>(id: Id, result: Option<T>) -> serde_json::Result<Response>
    where
        T: Serialize,
    {
        let serialized_result = match result {
            Some(r) => Some(serde_json::to_value(r)?),
            None => None,
        };
        Ok(Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: serialized_result,
            error: None,
        })
    }

    pub fn invalid_request(id: Id, source: &str) -> Response {
        Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(Error::invalid_request(source)),
        }
    }

    pub fn method_not_found(id: Id, method: &str) -> Response {
        Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(Error::method_not_found(method)),
        }
    }

    pub fn invalid_params(id: Id, reason: &str) -> Response {
        Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(Error::invalid_params(reason)),
        }
    }
}

impl FromStr for Response {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = to_string(&self).map_err(|_| fmt::Error)?;
        f.write_str(&payload)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClientEvent {
    Notification(Notification),
    Response(Response),
}

impl ClientEvent {
    pub fn is_notification(&self) -> bool {
        use self::ClientEvent::*;
        match self {
            Notification(_) => true,
            _ => false,
        }
    }

    pub fn is_response(&self) -> bool {
        use self::ClientEvent::*;
        match self {
            Response(_) => true,
            _ => false,
        }
    }
}

impl FromStr for ClientEvent {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl fmt::Display for ClientEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ClientEvent::*;
        let payload = match self {
            Notification(n) => to_string(n).map_err(|_| fmt::Error)?,
            Response(r) => to_string(r).map_err(|_| fmt::Error)?,
        };
        f.write_str(&payload)
    }
}

#[macro_export]
macro_rules! response {
    ($msg:ident, $call:expr) => {
        Response::new(
            $msg.id.clone(),
            match $msg.params() {
                Ok(Some(ref params)) => $call(params),
                Ok(None) => Err(JError::invalid_request("missing field: params")),
                Err(err) => Err(JError::invalid_params(&err.to_string())),
            },
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_request() {
        let data = r#"{
                    "jsonrpc": "2.0",
                    "id": 100,
                    "method": "func1",
                    "params": ["item 1", "item 2"]
                  }"#;
        let request: Request = data.parse().unwrap();
        assert_eq!(request.id, Id::Number(100));
        assert_eq!(request.method, "func1".to_string());
        let params = request.params::<Vec<String>>().unwrap();
        assert_eq!(
            params.unwrap(),
            vec!["item 1".to_string(), "item 2".to_string()]
        );
    }

    #[test]
    fn deserialize_notification() {
        let data = r#"{
                    "jsonrpc": "2.0",
                    "method": "func1",
                    "params": ["item 1", "item 2"]
                  }"#;
        let notification: Notification = data.parse().unwrap();
        assert_eq!(notification.method, "func1".to_string());
        let params = notification.params::<Vec<String>>().unwrap();
        assert_eq!(
            params.unwrap(),
            vec!["item 1".to_string(), "item 2".to_string()]
        );
    }

    #[test]
    fn deserialize_notification_no_params() {
        let data = r#"{
                    "jsonrpc": "2.0",
                    "method": "func1"
                  }"#;
        let notification: Notification = data.parse().unwrap();
        assert_eq!(notification.method, "func1".to_string());
        let params = notification.params::<Vec<String>>().unwrap();
        assert!(params.is_none());
    }

    #[test]
    fn deserialize_response() {
        let data = r#"{
                    "jsonrpc": "2.0",
                    "id": 100,
                    "result": ["item 1", "item 2"]
                  }"#;
        let response: Response = data.parse().unwrap();
        assert_eq!(response.id, Id::Number(100));
        let result = response.result::<Vec<String>>().unwrap();
        assert_eq!(
            result.unwrap(),
            vec!["item 1".to_string(), "item 2".to_string()]
        );
    }

    #[test]
    fn deserialize_response_error() {
        let data = r#"{
                    "jsonrpc": "2.0",
                    "id": 100,
                    "error": {"code": -32601, "message": "Method not found", "data": "more details"}
                  }"#;
        let response: Response = data.parse().unwrap();
        assert_eq!(response.id, Id::Number(100));
        let result = response.result::<Vec<String>>().unwrap();
        let error = result.err().unwrap();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found".to_string());
        let data = error.data::<String>().unwrap();
        assert_eq!(data, Some("more details".to_string()));
    }

    #[test]
    fn deserialize_client_events() {
        let data_notification = r#"{
                    "jsonrpc": "2.0",
                    "method": "func1"
                  }"#;
        let notification: ClientEvent = data_notification.parse().unwrap();
        assert!(notification.is_notification());

        let data_response = r#"{
                    "jsonrpc": "2.0",
                    "id": 100,
                    "result": ["item 1", "item 2"]
                  }"#;
        let response: ClientEvent = data_response.parse().unwrap();
        assert!(response.is_response());
    }
}
