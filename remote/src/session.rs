use std::env;
use std::fmt;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use regex::Regex;

lazy_static! {
    static ref RE_ADDR: Regex =
        Regex::new(r"^@(?P<address>\w+|\d+\.\d\.\d+\.\d+)?:(?P<port>\d+)$").unwrap();
}

#[cfg(unix)]
const USER_ENV_VAR: &'static str = "LOGNAME";
#[cfg(windows)]
const USER_ENV_VAR: &'static str = "USERNAME";

#[derive(Debug, PartialEq)]
pub enum ConnectionMode {
    Socket(PathBuf),
    Tcp(SocketAddr),
}

impl ConnectionMode {
    fn is_tcp(s: &str) -> bool {
        RE_ADDR.is_match(s)
    }
}

impl fmt::Display for ConnectionMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ConnectionMode::*;
        match self {
            Socket(path) => write!(f, "{}", path.file_name().unwrap().to_str().unwrap()),
            Tcp(addr) => write!(f, "@{}", addr),
        }
    }
}

impl FromStr for ConnectionMode {
    type Err = ::std::string::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::ConnectionMode::*;
        if let Some(caps) = RE_ADDR.captures(s) {
            let address = caps
                .name("address")
                .and_then(|m| Some(m.as_str()))
                .unwrap_or("127.0.0.1");
            let port = caps.name("port").unwrap().as_str();
            let sock_addr = format!("{}:{}", address, port)
                .parse()
                .expect("invalid address");
            Ok(Tcp(sock_addr))
        } else {
            Ok(Socket(s.into()))
        }
    }
}

pub struct Session {
    pub mode: ConnectionMode,
}

impl Session {
    fn build_root() -> PathBuf {
        let mut app_dir = env::temp_dir();
        app_dir.push("ced");
        app_dir.push(env::var(USER_ENV_VAR).unwrap());
        app_dir
    }

    pub fn from_name(name: &str) -> Session {
        let session_name = if ConnectionMode::is_tcp(name) {
            name.to_owned()
        } else {
            let mut session_path = Self::build_root();
            session_path.push(name);
            session_path.to_str().unwrap().to_owned()
        };
        Session {
            mode: session_name.parse().unwrap(),
        }
    }

    pub fn from_pid() -> Session {
        Self::from_name(&std::process::id().to_string())
    }

    pub fn list() -> Vec<String> {
        match fs::read_dir(Self::build_root()) {
            Ok(entries) => entries
                .filter_map(|entry| {
                    entry.ok().and_then(|e| {
                        e.path()
                            .file_name()
                            .and_then(|n| n.to_str().map(String::from))
                    })
                }).collect::<Vec<String>>(),
            Err(_) => Vec::new(),
        }
    }
}

impl fmt::Display for Session {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_parse() {
        assert_eq!(
            "/tmp/foo".parse::<ConnectionMode>().unwrap(),
            ConnectionMode::Socket("/tmp/foo".into())
        );
        assert_eq!(
            "@bar".parse::<ConnectionMode>().unwrap(),
            ConnectionMode::Socket("@bar".into())
        );
        assert_eq!(
            "@:8888".parse::<ConnectionMode>().unwrap(),
            ConnectionMode::Tcp("127.0.0.1:8888".parse().unwrap())
        );
        assert_eq!(
            "@1.2.3.4:8888".parse::<ConnectionMode>().unwrap(),
            ConnectionMode::Tcp("1.2.3.4:8888".parse().unwrap())
        );
    }

    #[test]
    fn connection_display() {
        assert_eq!(
            format!("{}", ConnectionMode::Socket("/tmp/foo".into())),
            "foo",
        );
        assert_eq!(format!("{}", ConnectionMode::Socket("@bar".into())), "@bar",);
        let unicode_s = "\u{1F37A} \u{2192} \u{1F603}";
        assert_eq!(
            format!("{}", ConnectionMode::Socket(unicode_s.into())),
            unicode_s
        );
        assert_eq!(
            format!("{}", ConnectionMode::Tcp("127.0.0.1:8888".parse().unwrap())),
            "@127.0.0.1:8888"
        );
    }
}
