/// Default controller for the `Model`.
pub trait DefaultController<T, U = T> {
    /// The request extractor.
    type Request;
    /// The response result.
    type Result;

    /// Creates a new model.
    async fn new(req: Self::Request) -> Self::Result;

    /// Deletes a model.
    async fn delete(req: Self::Request) -> Self::Result;

    /// Updates a model.
    async fn update(req: Self::Request) -> Self::Result;

    /// Views a model.
    async fn view(req: Self::Request) -> Self::Result;

    /// Lists models.
    async fn list(req: Self::Request) -> Self::Result;
}

#[cfg(any(feature = "actix", feature = "axum"))]
#[cfg(feature = "orm")]
use zino_core::{
    database::ModelAccessor, extension::JsonObjectExt, request::RequestContext,
    response::ExtractRejection, Map,
};

#[cfg(any(feature = "actix", feature = "axum"))]
#[cfg(feature = "orm")]
impl<T, U, M: ModelAccessor<T, U>> DefaultController<T, U> for M
where
    T: Default + std::fmt::Display + PartialEq + serde::de::DeserializeOwned,
    U: Default + std::fmt::Display + PartialEq,
{
    type Request = crate::Request;
    type Result = crate::Result;

    async fn new(mut req: Self::Request) -> Self::Result {
        let mut model = Self::new();
        let mut res: crate::Response = req.model_validation(&mut model).await?;

        let data = Map::data_entry(model.snapshot());
        model.upsert().await.extract(&req)?;
        res.set_data(&data);
        Ok(res.into())
    }

    async fn delete(req: Self::Request) -> Self::Result {
        let id = req.parse_param::<T>("id")?;
        Self::soft_delete_by_id(&id).await.extract(&req)?;

        let res = crate::Response::default().context(&req);
        Ok(res.into())
    }

    async fn update(mut req: Self::Request) -> Self::Result {
        let id = req.parse_param::<T>("id")?;
        let body: Map = req.parse_body().await?;
        let (validation, model) = Self::update_by_id(&id, body).await.extract(&req)?;
        let data = Map::data_entry(model.next_version_filters());
        let mut res = crate::Response::from(validation).context(&req);
        res.set_data(&data);
        Ok(res.into())
    }

    async fn view(req: Self::Request) -> Self::Result {
        let id = req.parse_param::<T>("id")?;
        let model = Self::fetch_by_id(&id).await.extract(&req)?;

        let data = Map::data_entry(model);
        let mut res = crate::Response::default().context(&req);
        res.set_data(&data);
        Ok(res.into())
    }

    async fn list(req: Self::Request) -> Self::Result {
        let mut query = Self::default_list_query();
        let mut res: crate::Response = req.query_validation(&mut query)?;
        let models = Self::fetch(&query).await.extract(&req)?;
        let data = Map::data_entries(models);
        res.set_data(&data);
        Ok(res.into())
    }
}
