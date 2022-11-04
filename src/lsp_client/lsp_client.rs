use std::error::Error;

use lsp_types::notification::Notification;
use lsp_types::request::Request;
use lsp_types::{
    lsp_request, InitializeError, InitializeParams, InitializeResult, SymbolInformation, Url,
    WorkspaceSymbolParams,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::{io::AsyncReadExt, process::Child};

use crate::lsp_client::json_rpc::_Response;

use super::json_rpc::{self, build_notification, build_request, JsonRpcError, Response};

pub struct StdIOLspClient {
    to_server: mpsc::UnboundedSender<Vec<u8>>,
    from_server: mpsc::UnboundedReceiver<Result<Value, Value>>,
}

impl StdIOLspClient {
    pub fn new(server: Child) -> Self {
        let (to_server, from_server) = start_io_threads(server);

        Self {
            to_server,
            from_server,
        }
    }

    pub async fn notify<P: Serialize>(&mut self, method: &str, params: &Option<P>) {
        let notification = build_notification(method, params);
        self.to_server
            .send(notification)
            .expect("failed to send request to server");
    }

    pub async fn call<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &Option<P>,
    ) -> Result<R, JsonRpcError> {
        let request = build_request(0, method, params);
        self.to_server
            .send(request)
            .expect("failed to send request to server");

        loop {
            if let Some(response) = self.from_server.recv().await {
                dbg!(&response);
                match response {
                    Ok(out) => {
                        if let Ok(result) = serde_json::from_value::<_Response<R>>(out) {
                            let response: Response<R> = result.into();

                            return response.response;
                        }
                    }
                    Err(err) => {
                        dbg!(err);
                    }
                }
            }
        }
    }

    pub async fn initialize(
        &mut self,
        params: &Option<InitializeParams>,
    ) -> Result<InitializeResult, JsonRpcError> {
        let result = self
            .call(lsp_types::request::Initialize::METHOD, params)
            .await?;

        self.notify(lsp_types::notification::Initialized::METHOD, params)
            .await;

        Ok(result)
    }

    pub async fn workspace_symbol(
        &mut self,
        params: &Option<WorkspaceSymbolParams>,
    ) -> Result<Vec<SymbolInformation>, JsonRpcError> {
        self.call(lsp_types::request::WorkspaceSymbol::METHOD, params)
            .await
    }
}

fn start_io_threads(
    mut server: Child,
) -> (
    mpsc::UnboundedSender<Vec<u8>>,
    mpsc::UnboundedReceiver<Result<Value, Value>>,
) {
    let (to_server, mut to_server_receiver) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        let stdin = server
            .stdin
            .as_mut()
            .take()
            .expect("failed to acquire stdout of server process");

        while let Some(buf) = to_server_receiver.recv().await {
            dbg!(std::str::from_utf8(&buf));
            stdin
                .write_all(&buf)
                .await
                .expect("failed to write to stdin");
        }
    });

    let from_server = {
        let (out_sender, responses) = mpsc::unbounded_channel::<Result<Value, Value>>();
        let err_sender = out_sender.clone();

        tokio::spawn(async move {
            let stdout = server
                .stdout
                .as_mut()
                .take()
                .expect("failed to acquire stdout of server process");

            while let Ok(buf) = json_rpc::get_next_response(stdout).await {
                dbg!(std::str::from_utf8(&buf));
                if let Ok(msg) = serde_json::from_slice::<Value>(&buf) {
                    out_sender
                        .send(Ok(msg))
                        .expect("failed to send response to from_server queue");
                }
            }
        });

        tokio::spawn(async move {
            let stderr = server
                .stderr
                .as_mut()
                .take()
                .expect("failed to acquire stderr of server process");

            let mut buf = vec![];
            while let Ok(byte) = stderr.read_u8().await {
                let err = std::str::from_utf8(&buf).unwrap();
                if let Some(char) = err.chars().last() {
                    if char == '\n' {
                        dbg!(&err);
                        err_sender
                            .send(Err(json!({ "err": err })))
                            .expect("failed to send error to from_server queue");
                        buf.clear();
                    }
                }

                buf.push(byte);
            }
        });
        responses
    };

    (to_server, from_server)
}
