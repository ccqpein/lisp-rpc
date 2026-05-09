use std::fmt;

use actix_web::{
    App, HttpResponse, HttpServer, Responder, ResponseError, http::StatusCode, post, web,
};
use lisp_rpc_rust_serializer::lisp_rpc_from_str;
use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub raw: String,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Req {
    pub time: i64,
    pub body: Option<Body>,
}

#[derive(Debug)]
pub struct AnyhowError(anyhow::Error);

// Implement Display so it can be printed
impl fmt::Display for AnyhowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Implement From to allow using the `?` operator on anyhow::Result
impl From<anyhow::Error> for AnyhowError {
    fn from(err: anyhow::Error) -> Self {
        AnyhowError(err)
    }
}

// Implement ResponseError to tell Actix how to convert it to an HTTP response
impl ResponseError for AnyhowError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().body(self.0.to_string())
    }

    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[post("/req-json")]
async fn handle_req_json(req: web::Json<Req>) -> Result<impl Responder, AnyhowError> {
    let req_data = req.into_inner();
    println!("Received JSON request: {:?}", req_data);

    let tt = req_data.time;

    Ok(HttpResponse::Ok().body(format!("Success, time is: {}", tt)))
}

#[post("/req-lisp")]
async fn handle_req_lisp(body: String) -> Result<impl Responder, AnyhowError> {
    println!("Received Lisp-RPC request: {}", body);

    let req = lisp_rpc_from_str::<Req>(&body)?;
    let tt = req.time;

    Ok(HttpResponse::Ok().body(format!("Success, time is: {}", tt)))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let host = "127.0.0.1";
    let port = 8080;
    println!("Starting demo server at http://{}:{}", host, port);
    println!("JSON endpoint: http://{}:{}/req-json", host, port);
    println!("Lisp endpoint: http://{}:{}/req-lisp", host, port);

    HttpServer::new(|| App::new().service(handle_req_json).service(handle_req_lisp))
        .bind((host, port))?
        .run()
        .await
}
