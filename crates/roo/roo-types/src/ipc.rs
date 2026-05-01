//! IPC (Inter-Process Communication) type definitions.
//!
//! Derived from `packages/types/src/ipc.ts`.

use serde::{Deserialize, Serialize};

/// IPC message type discriminator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IpcMessageType {
    Connect,
    Disconnect,
    Ack,
    TaskCommand,
    TaskEvent,
}

/// Origin of an IPC message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IpcOrigin {
    Client,
    Server,
}

/// Acknowledgement payload sent from server to client on connect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ack {
    pub client_id: String,
    pub pid: u32,
    pub ppid: u32,
}

/// Task command names that a client can send.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TaskCommandName {
    StartNewTask,
    CancelTask,
    CloseTask,
    ResumeTask,
    SendMessage,
    GetCommands,
    GetModes,
    GetModels,
    DeleteQueuedMessage,
}

/// Data payload for `StartNewTask`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartNewTaskData {
    pub configuration: serde_json::Value,
    pub text: String,
    pub images: Option<Vec<String>>,
    pub new_tab: Option<bool>,
}

/// Data payload for `ResumeTask`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResumeTaskData {
    pub data: String,
}

/// Data payload for `SendMessage`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageData {
    pub text: Option<String>,
    pub images: Option<Vec<String>>,
}

/// A task command sent from client to server.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "commandName", rename_all = "PascalCase")]
pub enum TaskCommand {
    StartNewTask {
        data: StartNewTaskData,
    },
    CancelTask,
    CloseTask,
    ResumeTask {
        data: String,
    },
    SendMessage {
        data: SendMessageData,
    },
    GetCommands,
    GetModes,
    GetModels,
    DeleteQueuedMessage {
        data: String,
    },
}

/// IPC message — the top-level envelope for all IPC communication.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum IpcMessage {
    Ack {
        origin: IpcOrigin,
        data: Ack,
    },
    TaskCommand {
        origin: IpcOrigin,
        client_id: String,
        data: TaskCommand,
    },
    TaskEvent {
        origin: IpcOrigin,
        relay_client_id: Option<String>,
        data: serde_json::Value,
    },
}
