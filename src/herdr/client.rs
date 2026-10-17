//! A blocking client for herdr's socket API: one newline-delimited JSON request and reply per connection.

use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::protocol::{ProcessInfo, Snapshot};

const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Workspace(String),
    Tab(String),
    Pane(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The socket couldn't be reached or the connection failed.
    Io(String),
    /// herdr answered with an error.
    Herdr {
        method: String,
        code: String,
        message: String,
    },
    /// herdr answered with something we couldn't read.
    Protocol(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(msg) | Error::Protocol(msg) => f.write_str(msg),
            Error::Herdr {
                method, message, ..
            } => write!(f, "{method}: {message}"),
        }
    }
}

impl std::error::Error for Error {}

/// What the plugin needs from herdr. Implemented by [`SocketClient`] and by test fakes.
pub trait Herdr {
    fn snapshot(&mut self) -> Result<Snapshot, Error>;
    fn process_info(&mut self, pane_id: &str) -> Result<ProcessInfo, Error>;
    fn focus(&mut self, target: &Target) -> Result<(), Error>;
    fn close_pane(&mut self, pane_id: &str) -> Result<(), Error>;
}

pub struct SocketClient {
    path: PathBuf,
}

impl SocketClient {
    pub fn new(path: PathBuf) -> Self {
        SocketClient { path }
    }

    /// `$HERDR_SOCKET_PATH`, else herdr's default socket.
    pub fn from_env() -> Self {
        let path = std::env::var_os("HERDR_SOCKET_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_default();
                home.join(".config/herdr/herdr.sock")
            });
        SocketClient::new(path)
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, Error> {
        let io =
            |e: std::io::Error| Error::Io(format!("herdr socket {}: {e}", self.path.display()));
        let mut stream = UnixStream::connect(&self.path).map_err(io)?;
        stream.set_read_timeout(Some(TIMEOUT)).map_err(io)?;
        stream.set_write_timeout(Some(TIMEOUT)).map_err(io)?;
        let line = json!({"id": "resource-manager", "method": method, "params": params});
        stream
            .write_all(format!("{line}\n").as_bytes())
            .map_err(io)?;
        let mut reply = String::new();
        BufReader::new(&stream).read_line(&mut reply).map_err(io)?;
        let mut msg: Value = serde_json::from_str(&reply)
            .map_err(|_| Error::Protocol(format!("{method}: unreadable reply")))?;
        if let Some(err) = msg.get("error") {
            return Err(Error::Herdr {
                method: method.to_string(),
                code: err["code"].as_str().unwrap_or_default().to_string(),
                message: err["message"]
                    .as_str()
                    .map_or_else(|| err.to_string(), str::to_string),
            });
        }
        Ok(msg["result"].take())
    }

    fn request_field<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
        field: &str,
    ) -> Result<T, Error> {
        let mut result = self.request(method, params)?;
        serde_json::from_value(result[field].take())
            .map_err(|e| Error::Protocol(format!("{method}: unexpected `{field}`: {e}")))
    }
}

impl Herdr for SocketClient {
    fn snapshot(&mut self) -> Result<Snapshot, Error> {
        self.request_field("session.snapshot", json!({}), "snapshot")
    }

    fn process_info(&mut self, pane_id: &str) -> Result<ProcessInfo, Error> {
        self.request_field(
            "pane.process_info",
            json!({"pane_id": pane_id}),
            "process_info",
        )
    }

    fn focus(&mut self, target: &Target) -> Result<(), Error> {
        let (method, params) = match target {
            Target::Pane(id) => ("pane.focus", json!({"pane_id": id})),
            Target::Tab(id) => ("tab.focus", json!({"tab_id": id})),
            Target::Workspace(id) => ("workspace.focus", json!({"workspace_id": id})),
        };
        self.request(method, params).map(drop)
    }

    fn close_pane(&mut self, pane_id: &str) -> Result<(), Error> {
        self.request("pane.close", json!({"pane_id": pane_id}))
            .map(drop)
    }
}
