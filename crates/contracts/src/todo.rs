//! Todos: items with an optional blocker list, owned by the Project.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "todo/")]
pub struct Todo {
    pub id: u32,
    pub title: String,
    pub body: String,
    pub done: bool,
    pub blockers: Vec<u32>,
    /// Derived: at least one blocker is not done. Never stored.
    pub blocked: bool,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "todo/")]
pub struct CreateParams {
    pub title: String,
    pub body: Option<String>,
    pub blockers: Option<Vec<u32>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "todo/")]
pub struct TodoId {
    pub id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "todo/")]
pub struct UpdateParams {
    pub id: u32,
    pub title: Option<String>,
    pub body: Option<String>,
}

/// Replaces the whole blocker list. A list that would make a cycle is an error.
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "todo/")]
pub struct SetBlockersParams {
    pub id: u32,
    pub blockers: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MCP clients (M1) read this schema to decide which fields they must send.
    #[test]
    fn m1_create_params_schema_requires_only_the_title() {
        let schema = serde_json::to_value(schemars::schema_for!(CreateParams)).unwrap();
        assert_eq!(schema["required"], serde_json::json!(["title"]));
    }
}
