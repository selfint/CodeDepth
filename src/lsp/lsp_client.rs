use log::{debug, error, warn};
use lsp_types::{
    notification::{Initialized, Notification},
    request::{
        CallHierarchyIncomingCalls, DocumentSymbolRequest, Initialize, Request, WorkspaceSymbol,
    },
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    DocumentSymbolParams, DocumentSymbolResponse, InitializeParams, InitializeResult,
    InitializedParams, PartialResultParams, SymbolInformation, TextDocumentIdentifier, Url,
    WorkDoneProgressParams, WorkspaceSymbolParams,
};
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Child,
    sync::{broadcast, mpsc},
};

use crate::lsp::json_rpc::{LspResponse, ResponseContents};

use super::json_rpc::{self, build_notification, build_request, LspError};

pub struct LspClient {
    to_server: mpsc::UnboundedSender<Vec<u8>>,
    from_server: broadcast::Receiver<Value>,
    request_count: usize,
}

impl LspClient {
    pub fn new(
        to_server: mpsc::UnboundedSender<Vec<u8>>,
        from_server: broadcast::Receiver<Value>,
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

        debug!(
            "Sending LSP notification:\n{}",
            std::str::from_utf8(&notification).unwrap()
        );

        self.to_server
            .send(notification)
            .expect("failed to send request to server");
    }

    pub async fn call<R: Request>(&mut self, params: &R::Params) -> Result<R::Result, LspError> {
        let request_id = self.request_count;
        let request = build_request::<R>(request_id, params);
        self.request_count += 1;

        debug!(
            "Sending LSP request:\n{}",
            std::str::from_utf8(&request).unwrap()
        );

        self.to_server
            .send(request)
            .expect("failed to send request to server");

        loop {
            let out = self
                .from_server
                .recv()
                .await
                .expect("Failed to recv from server");

            debug!(
                "Received LSP response:\n{}",
                serde_json::to_string_pretty(&out).unwrap()
            );

            let lsp_response = match serde_json::from_value::<LspResponse<R::Result>>(out) {
                Ok(response) => response,
                Err(err) => {
                    error!("Received malformed response, err: {}", err);
                    continue;
                }
            };

            let Some(response_id) = lsp_response.id else {
                warn!("Received unexpected response without id");
                continue;
            };

            if response_id != request_id {
                warn!(
                    "Received unexpected response id: {} (expected {})",
                    response_id, request_id
                );
                continue;
            }

            match lsp_response.response {
                ResponseContents::Result { result } => return Ok(result),
                ResponseContents::Error { error } => return Err(error),
                ResponseContents::UnknownResult { result: _ } => {
                    error!("Received unknown result type (this is probably fatal)");
                }
            }
        }
    }

    pub async fn initialize(
        &mut self,
        params: &InitializeParams,
    ) -> Result<InitializeResult, LspError> {
        let result = self.call::<Initialize>(params).await?;

        self.notify::<Initialized>(&InitializedParams {}).await;

        Ok(result)
    }

    pub async fn workspace_symbol(
        &mut self,
        query: &str,
    ) -> Result<Option<Vec<SymbolInformation>>, LspError> {
        let params = WorkspaceSymbolParams {
            query: query.to_string(),
            ..Default::default()
        };

        self.call::<WorkspaceSymbol>(&params).await
    }

    pub async fn document_symbol(
        &mut self,
        uri: Url,
    ) -> Result<Option<DocumentSymbolResponse>, LspError> {
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            partial_result_params: PartialResultParams::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.call::<DocumentSymbolRequest>(&params).await
    }

    pub async fn call_hierarchy_incoming_calls(
        &mut self,
        item: CallHierarchyItem,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, LspError> {
        let params = CallHierarchyIncomingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.call::<CallHierarchyIncomingCalls>(&params).await
    }
}

pub fn start_io_threads(
    mut server: Child,
) -> (mpsc::UnboundedSender<Vec<u8>>, broadcast::Receiver<Value>) {
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

    // log errors from server process
    tokio::spawn(async move {
        let stderr = server
            .stderr
            .as_mut()
            .take()
            .expect("failed to acquire stderr of server process");

        let mut buf = vec![];
        while let Ok(byte) = stderr.read_u8().await {
            buf.push(byte);

            let Ok(err) = std::str::from_utf8(&buf) else { continue };
            let Some(last_char) = err.chars().last() else { continue };

            if last_char == '\n' {
                error!("Got error from server process: {}", err);
                buf.clear();
            }
        }
    });

    let from_server = {
        let (to_responses, from_server) = broadcast::channel::<Value>(1_000_000);

        tokio::spawn(async move {
            let stdout = server
                .stdout
                .as_mut()
                .take()
                .expect("failed to acquire stdout of server process");

            while let Ok(buf) = json_rpc::get_next_response(stdout).await {
                if let Ok(msg) = serde_json::from_slice::<Value>(&buf) {
                    to_responses
                        .send(msg)
                        .expect("failed to send response to from_server queue");
                }
            }
        });

        from_server
    };

    (to_server, from_server)
}
