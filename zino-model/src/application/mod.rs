//! The `application` model and related services.

use crate::user::User;
use serde::{Deserialize, Serialize};
use zino_core::{
    auth::AccessKeyId,
    datetime::DateTime,
    error::Error,
    extension::JsonObjectExt,
    model::{Model, ModelHooks},
    validation::Validation,
    Map, Uuid,
};
use zino_derive::{DecodeRow, ModelAccessor, Schema};

#[cfg(feature = "tags")]
use crate::tag::Tag;

#[cfg(feature = "maintainer-id")]
use zino_core::auth::UserSession;

/// The `application` model.
#[derive(Debug, Clone, Default, Serialize, Deserialize, DecodeRow, Schema, ModelAccessor)]
#[serde(default)]
pub struct Application {
    // Basic fields.
    #[schema(read_only)]
    id: Uuid,
    #[schema(not_null)]
    name: String,
    #[cfg(feature = "namespace")]
    #[schema(default_value = "Application::model_namespace", index_type = "hash")]
    namespace: String,
    #[cfg(feature = "visibility")]
    #[schema(default_value = "Internal")]
    visibility: String,
    #[schema(default_value = "Active", index_type = "hash")]
    status: String,
    description: String,

    // Info fields.
    #[schema(reference = "User")]
    manager_id: Uuid, // user.id
    #[schema(not_null, unique, write_only)]
    access_key_id: String,
    #[cfg(feature = "tags")]
    #[schema(reference = "Tag", index_type = "gin")]
    tags: Vec<Uuid>, // tag.id, tag.namespace = "*:application"

    // Extensions.
    extra: Map,

    // Revisions.
    #[cfg(feature = "owner-id")]
    #[schema(reference = "User")]
    owner_id: Option<Uuid>, // user.id
    #[cfg(feature = "maintainer-id")]
    #[schema(reference = "User")]
    maintainer_id: Option<Uuid>, // user.id
    #[schema(read_only, default_value = "now", index_type = "btree")]
    created_at: DateTime,
    #[schema(default_value = "now", index_type = "btree")]
    updated_at: DateTime,
    version: u64,
    #[cfg(feature = "edition")]
    edition: u32,
}

impl Model for Application {
    #[inline]
    fn new() -> Self {
        Self {
            id: Uuid::now_v7(),
            access_key_id: AccessKeyId::new().to_string(),
            ..Self::default()
        }
    }

    fn read_map(&mut self, data: &Map) -> Validation {
        let mut validation = Validation::new();
        if let Some(result) = data.parse_uuid("id") {
            match result {
                Ok(id) => self.id = id,
                Err(err) => validation.record_fail("id", err),
            }
        }
        if let Some(name) = data.parse_string("name") {
            self.name = name.into_owned();
        }
        if let Some(description) = data.parse_string("description") {
            self.description = description.into_owned();
        }
        if let Some(result) = data.parse_uuid("manager_id") {
            match result {
                Ok(manager_id) => self.manager_id = manager_id,
                Err(err) => validation.record_fail("manager_id", err),
            }
        }
        #[cfg(feature = "tags")]
        if let Some(tags) = data.parse_array("tags") {
            self.tags = tags;
        }
        #[cfg(feature = "owner-id")]
        if let Some(result) = data.parse_uuid("owner_id") {
            match result {
                Ok(owner_id) => self.owner_id = Some(owner_id),
                Err(err) => validation.record_fail("owner_id", err),
            }
        }
        #[cfg(feature = "maintainer-id")]
        if let Some(result) = data.parse_uuid("maintainer_id") {
            match result {
                Ok(maintainer_id) => self.maintainer_id = Some(maintainer_id),
                Err(err) => validation.record_fail("maintainer_id", err),
            }
        }
        validation
    }
}

impl ModelHooks for Application {
    type Data = ();
    #[cfg(feature = "maintainer-id")]
    type Extension = UserSession<Uuid, String>;
    #[cfg(not(feature = "maintainer-id"))]
    type Extension = ();

    #[cfg(feature = "maintainer-id")]
    #[inline]
    async fn after_extract(&mut self, session: Self::Extension) -> Result<(), Error> {
        self.maintainer_id = Some(*session.user_id());
        Ok(())
    }

    #[cfg(feature = "maintainer-id")]
    #[inline]
    async fn before_validation(
        data: &mut Map,
        extension: Option<&Self::Extension>,
    ) -> Result<(), Error> {
        if let Some(session) = extension {
            data.upsert("maintainer_id", session.user_id().to_string());
        }
        Ok(())
    }
}

impl Application {
    /// Sets the `access_key_id`.
    #[inline]
    pub fn set_access_key_id(&mut self, access_key_id: AccessKeyId) {
        self.access_key_id = access_key_id.to_string();
    }
}
