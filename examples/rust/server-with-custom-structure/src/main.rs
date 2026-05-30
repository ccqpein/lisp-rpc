use lisp_rpc_rust_serializer::*;
use lisp_rpc_rust_server::*;
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind};

use actix_web::{App, HttpResponse, HttpServer, Responder, post, web};

/// some data that exist before
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SomeData {
    id: i64,
    data: String,
}

// impl RPCType with impl_to_rpc! from lisp_rpc_rust_server and lisp_rpc_rust_serializer

// the handler function arguments **has to be** RPCType::RPC
impl_to_rpc!(SomeData, RPCType::RPC("some-data".to_string()));

// if some type is map type, need to call register_global_map_type
// pub fn init() {
//     register_global_map_type("SomeData")
// }

// server impl below

#[post("/rpc")]
async fn rpc_handler(body: String, server: web::Data<RPCServer>) -> impl Responder {
    match server.handle(&body) {
        Ok(response) => HttpResponse::Ok().body(response),
        Err(e) => HttpResponse::BadRequest().body(format!("RPC Error: {}", e)),
    }
}

/// A simple hello handler for testing
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello from Lisp-RPC Server!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // call the init for register the right map type
    //init();

    // 1. Setup the RPC Engine
    // RPCServer internal is already Arc-wrapped, so it's cheap to clone
    let server = RPCServer::new()
        .register::<SomeData, _>(|gb: SomeData| {
            println!("Received BookInfo via Actix: {:?}", gb);
            Ok(format!("Processed book: {:?}", gb.serialize_lisp()))
        })
        .map_err(|e| Error::new(ErrorKind::Other, e))?;

    println!("Starting Actix-web RPC Server on http://127.0.0.1:8080");

    // 2. Standard Actix-web Server Setup
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(server.clone()))
            .route("/", web::get().to(hello))
            .service(rpc_handler)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
