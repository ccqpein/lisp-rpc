use anyhow::{Result, anyhow};
use lisp_rpc_rust_serializer::lisp_rpc_from_str;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use super::*;

/// A trait that captures the relationship between a request type T and its response.
pub trait RpcFunc<T>: Send + Sync + 'static {
    type Resp: Serialize + ToRPCType + 'static;
    fn call(&self, req: T) -> Result<Self::Resp>;
}

impl<T, R, F> RpcFunc<T> for F
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + 'static,
    R: Serialize + ToRPCType + 'static,
    F: Fn(T) -> Result<R> + Send + Sync + 'static,
{
    type Resp = R;
    fn call(&self, req: T) -> Result<Self::Resp> {
        (self)(req)
    }
}

/// The type-erased handler trait
pub trait RpcHandler: Send + Sync {
    fn handle(&self, raw_data: &str) -> Result<Box<dyn ToRPCType>>;
}

/// A concrete handler that knows its own request type T
struct Handler<T, F> {
    func: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> RpcHandler for Handler<T, F>
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + 'static,
    F: RpcFunc<T>,
{
    fn handle(&self, raw_data: &str) -> Result<Box<dyn ToRPCType>> {
        let req: T =
            lisp_rpc_from_str(raw_data).map_err(|e| anyhow!("Deserialization failed: {}", e))?;
        let resp = self.func.call(req)?;
        Ok(Box::new(resp))
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
        F: RpcFunc<T>,
    {
        // has to be RPCType::RPC
        let command = match <T as ToRPCType>::to_rpc_type() {
            RPCType::RPC(s) => s,
            _ => anyhow::bail!("Handler function argument has to be RPCType::RPC"),
        };

        let handler = Handler {
            func,
            _phantom: std::marker::PhantomData,
        };

        Arc::get_mut(&mut self.handlers)
            .unwrap()
            .insert(command, Box::new(handler));

        Ok(self)
    }

    /// Dispatch a raw Lisp RPC string to the appropriate handler
    pub fn handle(&self, raw_data: &str) -> Result<String> {
        // 1. Extract the command name from the Lisp string (e.g., "(command-name ...)")
        let command =
            extract_command_name(raw_data).ok_or_else(|| anyhow!("Invalid RPC format"))?;

        // 2. Find the registered handler
        let handler = self
            .handlers
            .get(&command)
            .ok_or_else(|| anyhow!("Unknown command: {}", command))?;

        // 3. Execute the handler to get the trait object
        let resp_obj = handler.handle(raw_data)?;

        // 4. Serialize the response using the trait object's method
        resp_obj.serialize_lisp()
    }
}

/// Helper to get the first symbol from "(symbol ...)"
fn extract_command_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches('(');
    trimmed.split_whitespace().next().map(|s| s.to_string())
}
