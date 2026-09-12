use std::env;
use std::time::Duration;

use futures_util::StreamExt;
use lisp_rpc_rust_parser::TypeValue;
use lisp_rpc_rust_raw_data::{Data, IntoData, RawDataGenerator};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::io::ReaderStream;

/// Helper to wrap a string as [`Data::Value`].
fn str_data(s: &str) -> Data {
    Data::Value(TypeValue::String(s.to_string()))
}

/// Handles an incoming TCP connection by wrapping its read half with `RawDataGenerator`.
///
/// Unlike standard length-prefixed TCP protocols (e.g. `server-with-tcp`),
/// `RawDataGenerator` parses raw Lisp S-expression data structures directly
/// from an asynchronous byte stream without requiring any framing headers or length prefixes.
async fn handle_connection(socket: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let peer_addr = socket.peer_addr()?;
    println!("[Server] Client connected from: {}", peer_addr);

    let (reader, mut writer) = socket.into_split();

    // 1. Convert the Tokio AsyncRead into a Stream of byte chunks (Result<Bytes, io::Error>)
    let byte_stream = ReaderStream::new(reader);

    // 2. Wrap the byte stream in RawDataGenerator to yield parsed `Data` items asynchronously
    let mut data_generator = RawDataGenerator::new(byte_stream);

    // 3. Asynchronously iterate over each `Data` item as soon as it is fully parsed
    while let Some(item_res) = data_generator.next().await {
        match item_res {
            Ok(data) => {
                println!("\n[Server] Received Data item: {}", data.to_string());

                match &data {
                    Data::Data(expr) => {
                        println!("  -> S-expression name: {}", expr.get_name());

                        match expr.get_name() {
                            "login" => {
                                if let Some(Data::Value(TypeValue::String(user))) = expr.get("user") {
                                    println!("  -> Action: User '{}' logged in", user);
                                }
                                if let Some(Data::Value(TypeValue::String(role))) = expr.get("role") {
                                    println!("  -> Role: {}", role);
                                }
                            }
                            "telemetry" => {
                                if let Some(Data::Value(TypeValue::String(device))) = expr.get("device") {
                                    println!("  -> Device: {}", device);
                                }
                                if let Some(val) = expr.get("temp") {
                                    println!("  -> Temperature: {:?}", val);
                                }
                            }
                            "message" => {
                                if let Some(Data::Value(TypeValue::String(msg))) = expr.get("text") {
                                    println!("  -> Message text: {}", msg);
                                }
                            }
                            _ => {
                                println!("  -> Other expression received");
                            }
                        }

                        // Send back an acknowledgment S-expression to the client
                        let op_val = str_data(expr.get_name());
                        let ok_val = str_data("ok");
                        let ack = Data::new(
                            "ack",
                            [
                                ("op", &op_val as &dyn IntoData),
                                ("status", &ok_val as &dyn IntoData),
                            ]
                            .into_iter(),
                        )?;

                        let ack_bytes = ack.to_string();
                        writer.write_all(ack_bytes.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    _ => {
                        println!("  -> Non-expr data received: {:?}", data);
                    }
                }
            }
            Err(e) => {
                eprintln!("[Server] Error while parsing incoming stream: {}", e);
                break;
            }
        }
    }

    println!("[Server] Client {} disconnected cleanly.", peer_addr);
    Ok(())
}

/// Runs the TCP server listening on the specified address.
async fn run_server(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    println!("=== Streaming Raw Data TCP Server ===");
    println!("Listening on: {}", addr);
    println!("Waiting for client streaming connections...\n");

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                eprintln!("[Server] Connection handler error: {}", e);
            }
        });
    }
}

/// Runs a client that sends multiple S-expressions in arbitrary chunk fragments
/// to demonstrate `RawDataGenerator` assembling data across TCP chunk boundaries.
async fn run_client(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Streaming Raw Data Client ===");
    let mut socket = TcpStream::connect(addr).await?;
    println!("Connected to server at {}", addr);

    // Prepare several Data structures
    let user_val = str_data("alice");
    let role_val = str_data("admin");
    let login_data = Data::new(
        "login",
        [
            ("user", &user_val as &dyn IntoData),
            ("role", &role_val as &dyn IntoData),
        ]
        .into_iter(),
    )?;

    let device_val = str_data("sensor-42");
    let telemetry_data = Data::new(
        "telemetry",
        [
            ("device", &device_val as &dyn IntoData),
            ("temp", &IntoData::into_rpc_data(&24_i32)),
        ]
        .into_iter(),
    )?;

    let text_val = str_data("Streaming Lisp-RPC with UTF-8: 你好, 🚀");
    let message_data = Data::new(
        "message",
        [
            ("text", &text_val as &dyn IntoData),
            ("seq", &1_i32 as &dyn IntoData),
        ]
        .into_iter(),
    )?;

    println!("\n[Client] Sending 3 raw S-expressions over TCP stream in distinct chunks:");
    println!("  1. (login ...) complete in one chunk");
    println!("  2. (telemetry ...) split across token boundaries");
    println!("  3. (message ...) split right inside a string literal with multi-byte UTF-8\n");

    let telem_str = format!("{} ", telemetry_data.to_string());
    let (telem_part1, telem_part2) = telem_str.split_at(telem_str.find(":temp").unwrap());

    let msg_str = message_data.to_string();
    let split_pos = msg_str.find("你好").unwrap();
    let (msg_part1, msg_part2) = msg_str.split_at(split_pos);

    let chunks = vec![
        // 1. First expression: complete in one chunk
        format!("{} ", login_data.to_string()),

        // 2. Second expression: split across token boundaries
        telem_part1.to_string(),
        telem_part2.to_string(),

        // 3. Third expression: split inside string literal with UTF-8
        msg_part1.to_string(),
        msg_part2.to_string(),
    ];

    for (i, chunk) in chunks.iter().enumerate() {
        socket.write_all(chunk.as_bytes()).await?;
        println!("  [Client] Sent chunk #{} ({} bytes): {:?}", i + 1, chunk.len(), chunk);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Wrap the response half of socket with RawDataGenerator to stream server acks
    let (reader, _) = socket.into_split();
    let resp_stream = ReaderStream::new(reader);
    let mut resp_generator = RawDataGenerator::new(resp_stream);

    println!("\n[Client] Reading server acknowledgments via RawDataGenerator:");
    while let Some(resp_item) = resp_generator.next().await {
        match resp_item {
            Ok(resp_data) => {
                println!("  [Client] Received ack: {}", resp_data.to_string());
                if let Data::Data(expr) = &resp_data {
                    if let Some(Data::Value(TypeValue::String(op))) = expr.get("op") {
                        if op == "message" {
                            // Received all 3 expected acks
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("  [Client] Error reading response: {}", e);
                break;
            }
        }
    }

    println!("\n[Client] Streaming finished successfully.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let addr = "127.0.0.1:8082";

    if args.len() > 1 && args[1] == "--server" {
        run_server(addr).await?;
        return Ok(());
    }

    if args.len() > 1 && args[1] == "--client" {
        run_client(addr).await?;
        return Ok(());
    }

    // Default: run an automated end-to-end demonstration
    println!("No --server or --client flag provided. Running automated demo...\n");

    // Spawn server in background
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let bound_addr = listener.local_addr()?.to_string();

    tokio::spawn(async move {
        loop {
            if let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let _ = handle_connection(socket).await;
                });
            }
        }
    });

    // Run client against the ephemeral server port
    run_client(&bound_addr).await?;

    Ok(())
}
