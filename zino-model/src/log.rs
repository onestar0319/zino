use serde::{Deserialize, Serialize};
use zino_core::{datetime::DateTime, model::Model, request::Validation, Map, Uuid};
use zino_derive::Schema;

/// The log model.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Schema)]
#[serde(rename_all = "snake_case")]
#[serde(default)]
pub struct Log {
    // Basic fields.
    id: Uuid,
    #[schema(not_null, index = "text")]
    name: String,
    #[schema(default = "Log::model_namespace", index = "hash")]
    namespace: String,
    #[schema(default = "internal")]
    visibility: String,
    #[schema(default = "active", index = "hash")]
    status: String,
    #[schema(index = "text")]
    description: String,

    // Info fields.
    #[schema(not_null)]
    service: String,
    server_host: String,
    client_ip: String,
    topic: String,
    level: String,
    #[schema(index = "text")]
    message: String,
    source: String,
    #[schema(index = "btree")]
    recorded_at: DateTime,

    // Extensions.
    content: Map,
    metrics: Map,
    extras: Map,

    // Revisions.
    manager_id: Uuid,    // user.id
    maintainer_id: Uuid, // user.id
    #[schema(default = "now", index = "btree")]
    created_at: DateTime,
    #[schema(default = "now", index = "btree")]
    updated_at: DateTime,
    version: u64,
    edition: u32,
}

impl Model for Log {
    fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            ..Self::default()
        }
    }

    fn read_map(&mut self, data: &Map) -> Validation {
        let mut validation = Validation::new();
        if let Some(result) = Validation::parse_uuid(data.get("id")) {
            match result {
                Ok(id) => self.id = id,
                Err(err) => validation.record_fail("id", err),
            }
        }
        if let Some(name) = Validation::parse_string(data.get("name")) {
            self.name = name;
        }
        if self.name.is_empty() {
            validation.record_fail("name", "should be nonempty");
        }
        validation
    }
}

super::impl_model_accessor!(
    Log,
    id,
    name,
    namespace,
    visibility,
    status,
    description,
    content,
    metrics,
    extras,
    manager_id,
    maintainer_id,
    created_at,
    updated_at,
    version,
    edition
);
