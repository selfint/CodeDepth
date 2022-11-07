use lsp_types::{
    notification::{Initialized, Notification},
    request::{
        CallHierarchyIncomingCalls, DocumentSymbolRequest, Initialize, Request, WorkspaceSymbol,
    },
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, DocumentSymbolParams,
    DocumentSymbolResponse, InitializeParams, InitializeResult, InitializedParams,
    SymbolInformation, WorkspaceSymbolParams,
};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Child,
    sync::mpsc,
};

use crate::lsp::json_rpc::_Response;

use super::json_rpc::{self, build_notification, build_request, JsonRpcError, Response};

pub struct LspClient {
    to_server: mpsc::UnboundedSender<Vec<u8>>,
    from_server: mpsc::UnboundedReceiver<Result<Value, Value>>,
    request_count: usize,
}

impl LspClient {
    pub fn new(
        to_server: mpsc::UnboundedSender<Vec<u8>>,
        from_server: mpsc::UnboundedReceiver<Result<Value, Value>>,
    ) -> Self {
        Self {
            to_server,
            from_server,
            request_count: 0,
        }
    }

    pub fn stdio_client(server: Child) -> LspClient {
        let (to_server, from_server) = start_io_threads(server);

        LspClient::new(to_server, from_server)
    }

    pub async fn notify<N: Notification>(&mut self, params: &N::Params) {
        let notification = build_notification::<N>(params);
        self.to_server
            .send(notification)
            .expect("failed to send request to server");
    }

    pub async fn call<R: Request>(
        &mut self,
        params: &R::Params,
    ) -> Result<R::Result, JsonRpcError> {
        let request = build_request::<R>(self.request_count, params);
        self.request_count += 1;

        self.to_server
            .send(request)
            .expect("failed to send request to server");

        loop {
            if let Some(response) = self.from_server.recv().await {
                dbg!(&response);
                match response {
                    Ok(out) => match serde_json::from_value::<_Response<R::Result>>(out) {
                        Ok(result) => {
                            let response: Response<R::Result> = result.into();

                            return response.response;
                        }
                        Err(err) => {
                            eprintln!("got unexpected response type, or failed to deserialize response, err:\n{}", err);
                        }
                    },
                    Err(err) => {
                        eprintln!("got error event, err:\n{}", err);
                    }
                }
            }
        }
    }

    pub async fn initialize(
        &mut self,
        params: &InitializeParams,
    ) -> Result<InitializeResult, JsonRpcError> {
        let result = self.call::<Initialize>(params).await?;

        self.notify::<Initialized>(&InitializedParams {}).await;

        Ok(result)
    }

    pub async fn workspace_symbol(
        &mut self,
        params: &WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>, JsonRpcError> {
        self.call::<WorkspaceSymbol>(params).await
    }

    pub async fn document_symbol(
        &mut self,
        params: &DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>, JsonRpcError> {
        self.call::<DocumentSymbolRequest>(params).await
    }

    pub async fn call_hierarchy_incoming_calls(
        &mut self,
        params: &CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, JsonRpcError> {
        self.call::<CallHierarchyIncomingCalls>(params).await
    }
}

pub fn start_io_threads(
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
