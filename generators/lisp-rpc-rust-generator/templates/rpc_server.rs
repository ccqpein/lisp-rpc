use anyhow::{Result, anyhow};
use lisp_rpc_rust_serializer::lisp_rpc_from_str;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use super::*;

/// The type-erased handler trait
pub trait RpcHandler: Send + Sync {
    fn handle(&self, raw_data: &str) -> Result<String>;
}

/// A concrete handler that knows its own type T
struct TypedHandler<T, F> {
    func: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> RpcHandler for TypedHandler<T, F>
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType,
    F: Fn(T) -> Result<String> + Send + Sync,
{
    fn handle(&self, raw_data: &str) -> Result<String> {
        let req: T =
            lisp_rpc_from_str(raw_data).map_err(|e| anyhow!("Deserialization failed: {}", e))?;
        (self.func)(req)
    }
}

/// RPCServer manages a registry of handlers and dispatches incoming raw Lisp RPC strings
#[derive(Clone)]
pub struct RPCServer {
    pub handlers: Arc<HashMap<String, Box<dyn RpcHandler>>>,
}

impl RPCServer {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(HashMap::new()),
        }
    }

    /// Register a handler for a specific command
    pub fn register<T, F>(mut self, func: F) -> Result<Self>
    where
        T: DeserializeOwned + Debug + Send + Sync + ToRPCType + 'static,
        F: Fn(T) -> Result<String> + Send + Sync + 'static,
    {
        // has to be RPCType::RPC
        let command = match <T as ToRPCType>::to_rpc_type() {
            RPCType::RPC(s) => s,
            _ => anyhow::bail!("handler function argument has to be RPCType::RPC"),
        };

        let handler = TypedHandler {
            func,
            _phantom: std::marker::PhantomData,
        };
        Arc::get_mut(&mut self.handlers)
            .unwrap()
            .insert(command, Box::new(handler));

        Ok(self)
    }

    /// Dispatch a raw Lisp RPC string to the appropriate handler
    pub fn dispatch(&self, raw_data: &str) -> Result<String> {
        // 1. Extract the command name from the Lisp string (e.g., "(command-name ...)")
        let command =
            extract_command_name(raw_data).ok_or_else(|| anyhow!("Invalid RPC format"))?;

        // 2. Find the registered handler
        let handler = self
            .handlers
            .get(&command)
            .ok_or_else(|| anyhow!("Unknown command: {}", command))?;

        // 3. Execute the handler
        handler.handle(raw_data)
    }
}

/// Helper to get the first symbol from "(symbol ...)"
pub fn extract_command_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches('(');
    trimmed.split_whitespace().next().map(|s| s.to_string())
}
