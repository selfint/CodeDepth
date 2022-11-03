use lsp_types::request::Request;
use lsp_types::{lsp_request, InitializeError, InitializeParams, InitializeResult, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::{io::AsyncReadExt, process::Child};

use crate::lsp_client::json_rpc::_Response;

use super::json_rpc::{self, build_request};

pub struct StdIOLspClient {
    root: Url,
    to_server: mpsc::UnboundedSender<Vec<u8>>,
    from_server: mpsc::UnboundedReceiver<Result<Value, Value>>,
}

impl StdIOLspClient {
    pub fn new(server: Child, root: Url) -> Self {
        let (to_server, from_server) = start_io_threads(server);

        Self {
            root,
            to_server,
            from_server,
        }
    }

    pub async fn initialize(
        &mut self,
        params: &Option<InitializeParams>,
    ) -> Result<InitializeResult, InitializeError> {
        let request = build_request(0, lsp_types::request::Initialize::METHOD, params);
        self.to_server
            .send(request)
            .expect("failed to send request to server");

        loop {
            if let Some(response) = self.from_server.recv().await {
                match response {
                    Ok(out) => {
                        if let Ok(result) =
                            serde_json::from_value::<_Response<InitializeResult>>(out)
                        {
                            return Ok(result.result.expect("didn't get initialize result"));
                        }
                    }
                    Err(err) => {
                        dbg!(err);
                    }
                }
            }
        }
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

        while let Some(response) = to_server_receiver.recv().await {
            stdin
                .write_all(&response)
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
                buf.push(byte);

                if let Ok(msg) = serde_json::from_slice::<Value>(&buf) {
                    err_sender
                        .send(Err(msg))
                        .expect("failed to send error to from_server queue");
                    buf.clear();
                }
            }
        });
        responses
    };

    (to_server, from_server)
}
