use lisp_rpc_rust_serializer::*;
use lisp_rpc_rust_server::*;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// some data that exist before
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SomeData {
    id: i64,
    data: String,
}

// impl RPCType with impl_to_rpc! from lisp_rpc_rust_server and lisp_rpc_rust_serializer
impl_to_rpc!(SomeData, RPCType::RPC("some-data".to_string()));

async fn handle_connection(mut socket: TcpStream, server: RPCServer) {
    let mut buffer = vec![0; 4096];
    loop {
        // Read 4 bytes length prefix
        let mut len_bytes = [0u8; 4];
        match socket.read_exact(&mut len_bytes).await {
            Ok(_) => {}
            Err(e) => {
                if e.kind() != ErrorKind::UnexpectedEof {
                    eprintln!("Error reading length prefix: {}", e);
                }
                break;
            }
        }
        let len = u32::from_be_bytes(len_bytes) as usize;
        if len > buffer.len() {
            buffer.resize(len, 0);
        }

        // Read the exact request body bytes
        if let Err(e) = socket.read_exact(&mut buffer[..len]).await {
            eprintln!("Error reading request body of length {}: {}", len, e);
            break;
        }

        let body_str = match std::str::from_utf8(&buffer[..len]) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Invalid UTF-8 in request: {}", e);
                break;
            }
        };

        println!("Server received request: {}", body_str);

        // Process request using RPCServer
        let response = match server.handle(body_str) {
            Ok(res) => res,
            Err(e) => format!("RPC Error: {}", e),
        };

        println!("Server sending response: {}", response);

        // Write response back with length prefix
        let response_bytes = response.as_bytes();
        let resp_len_bytes = (response_bytes.len() as u32).to_be_bytes();
        if let Err(e) = socket.write_all(&resp_len_bytes).await {
            eprintln!("Error writing response length: {}", e);
            break;
        }
        if let Err(e) = socket.write_all(response_bytes).await {
            eprintln!("Error writing response: {}", e);
            break;
        }
    }
}

async fn run_client() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:8081").await?;
    println!("Client connected to TCP server!");

    let test_data = SomeData {
        id: 42,
        data: "Hello from TCP Client!".to_string(),
    };

    // Serialize test data using serialize_lisp
    let request_str = test_data.serialize_lisp()?;
    println!("Client sending request: {}", request_str);

    let request_bytes = request_str.as_bytes();
    let len_bytes = (request_bytes.len() as u32).to_be_bytes();

    stream.write_all(&len_bytes).await?;
    stream.write_all(request_bytes).await?;

    // Read response length
    let mut resp_len_bytes = [0u8; 4];
    stream.read_exact(&mut resp_len_bytes).await?;
    let resp_len = u32::from_be_bytes(resp_len_bytes) as usize;

    let mut resp_bytes = vec![0u8; resp_len];
    stream.read_exact(&mut resp_bytes).await?;
    let resp_str = std::str::from_utf8(&resp_bytes)?;

    println!("Client received response: {}", resp_str);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--client" {
        run_client().await?;
        return Ok(());
    }

    // 1. Setup the RPC Engine
    let server = RPCServer::new()
        .register::<SomeData, _>(|gb: SomeData| {
            println!("Received BookInfo via TCP: {:?}", gb);
            Ok(format!("Processed book: {:?}", gb.serialize_lisp()))
        })
        .map_err(|e| Error::new(ErrorKind::Other, e))?;

    let addr = "127.0.0.1:8081";
    let listener = TcpListener::bind(addr).await?;
    println!("Starting TCP RPC Server on {}", addr);

    loop {
        let (socket, _) = listener.accept().await?;
        let server_clone = server.clone();
        tokio::spawn(async move {
            handle_connection(socket, server_clone).await;
        });
    }
}
