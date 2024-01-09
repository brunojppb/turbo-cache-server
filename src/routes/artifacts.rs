use std::collections::HashMap;

use actix_web::{
    web::{Bytes, Data, Query},
    HttpRequest, HttpResponse, Responder,
};

use crate::storage::Storage;

pub async fn put_file(req: HttpRequest, storage: Data<Storage>, body: Bytes) -> impl Responder {
    let hash = match req.match_info().get("hash") {
        Some(h) => h,
        None => return HttpResponse::BadRequest().finish(),
    };

    let query_string = Query::<HashMap<String, String>>::from_query(req.query_string()).unwrap();
    let default_team_name = "no_team".to_owned();
    let team_name = query_string.get("slug").unwrap_or(&default_team_name);

    let file_path = format!("/{}/{}", team_name, hash);
    match storage.put_file(&file_path, &body).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            println!("Something went wrong {}", e);
            HttpResponse::BadRequest().finish()
        }
    }
}
