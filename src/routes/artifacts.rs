use actix_web::{HttpRequest, HttpResponse, Responder};

pub async fn put_file(req: HttpRequest) -> impl Responder {
    let hash = match req.match_info().get("hash") {
        Some(h) => h,
        None => return HttpResponse::BadRequest().finish(),
    };

    HttpResponse::Ok().finish()
}
