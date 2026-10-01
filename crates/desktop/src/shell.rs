use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Project {
    pub name: String,
    pub path: String,
}

pub struct Shell;

impl Shell {
    pub async fn subscribe(
        &self,
        _channel: tauri::ipc::Channel<contracts::Event>,
    ) -> Result<(), rpc::RpcError> {
        todo!()
    }
}
