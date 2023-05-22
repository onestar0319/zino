use fluent::fluent_args;
use serde_json::json;
use std::time::Instant;
use zino::{prelude::*, Request, Response, Result};
use zino_model::User;

pub async fn new(mut req: Request) -> Result {
    let mut user = User::new();
    let mut res: Response = req.model_validation(&mut user).await?;

    let user_name = user.name().to_owned();
    user.upsert().await.extract(&req)?;

    let args = fluent_args![
        "name" => user_name
    ];
    let user_intro = req.translate("user-intro", Some(args)).extract(&req)?;
    let data = json!({
        "method": req.request_method().as_ref(),
        "path": req.request_path(),
        "user_intro": user_intro,
    });
    res.set_data(&data);
    Ok(res.into())
}

pub async fn update(mut req: Request) -> Result {
    let user_id: Uuid = req.parse_param("id")?;
    let body: Map = req.parse_body().await?;
    let (validation, user) = User::update_by_id(&user_id, body).await.extract(&req)?;
    let mut res = Response::from(validation).context(&req);

    let data = Map::data_entry(user.next_version_filters());
    res.set_data(&data);
    Ok(res.into())
}

pub async fn list(req: Request) -> Result {
    let mut query = User::default_list_query();
    let mut res: Response = req.query_validation(&mut query)?;
    let users = User::find(&query).await.extract(&req)?;
    let data = Map::data_entries(users);
    res.set_data(&data);
    Ok(res.into())
}

pub async fn view(req: Request) -> Result {
    let user_id: Uuid = req.parse_param("id")?;

    let db_query_start_time = Instant::now();
    let user: Map = User::find_by_id(&user_id).await.extract(&req)?;
    let db_query_duration = db_query_start_time.elapsed();

    let data = Map::data_entry(user);
    let mut res = Response::default().context(&req);
    res.record_server_timing("db", None, Some(db_query_duration));
    res.set_data(&data);
    Ok(res.into())
}
