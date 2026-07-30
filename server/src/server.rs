use anyhow::{Result, anyhow};
use lisp_rpc_rust_serializer::lisp_rpc_from_str;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::*;

/// A trait that captures the relationship between a request type T and its response.
pub trait RpcFunc<T, R>: Send + Sync + 'static
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn<Return = R> + 'static,
    R: Serialize + ToRPCType + 'static,
{
    fn call(&self, req: T) -> Result<R>;
}

impl<T, R, F> RpcFunc<T, R> for F
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn<Return = R> + 'static,
    R: Serialize + ToRPCType + 'static,
    F: Fn(T) -> Result<R> + Send + Sync + 'static,
{
    fn call(&self, req: T) -> Result<R> {
        (self)(req)
    }
}

/// A trait that captures the relationship between a request type T and its async response.
pub trait AsyncRpcFunc<T, R>: Send + Sync + 'static
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn<Return = R> + 'static,
    R: Serialize + ToRPCType + 'static,
{
    type Fut: Future<Output = Result<R>> + Send + 'static;
    fn call(&self, req: T) -> Self::Fut;
}

impl<T, R, F, Fut> AsyncRpcFunc<T, R> for F
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn<Return = R> + 'static,
    R: Serialize + ToRPCType + 'static,
    Fut: Future<Output = Result<R>> + Send + 'static,
    F: Fn(T) -> Fut + Send + Sync + 'static,
{
    type Fut = Fut;
    fn call(&self, req: T) -> Self::Fut {
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
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn + 'static,
    T::Return: Serialize + ToRPCType + 'static,
    F: RpcFunc<T, T::Return>,
{
    fn handle(&self, raw_data: &str) -> Result<Box<dyn ToRPCType>> {
        let req: T =
            lisp_rpc_from_str(raw_data).map_err(|e| anyhow!("Deserialization failed: {}", e))?;
        let resp = self.func.call(req)?;
        Ok(Box::new(resp))
    }
}

/// The type-erased async handler trait
pub trait AsyncRpcHandler: Send + Sync {
    fn handle(
        &self,
        raw_data: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn ToRPCType>>> + Send>>;
}

/// A concrete async handler that knows its own request type T
struct AsyncHandler<T, F> {
    func: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> AsyncRpcHandler for AsyncHandler<T, F>
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn + 'static,
    T::Return: Serialize + ToRPCType + 'static,
    F: AsyncRpcFunc<T, T::Return>,
{
    fn handle(
        &self,
        raw_data: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn ToRPCType>>> + Send>> {
        let req_res =
            lisp_rpc_from_str(raw_data).map_err(|e| anyhow!("Deserialization failed: {}", e));
        match req_res {
            Ok(req) => {
                let fut = self.func.call(req);
                Box::pin(async move {
                    let resp = fut.await?;
                    Ok(Box::new(resp) as Box<dyn ToRPCType>)
                })
            }
            Err(e) => Box::pin(async move { Err(e) }),
        }
    }
}

/// RPCServer manages a registry of handlers and dispatches incoming raw Lisp RPC strings
#[derive(Clone)]
pub struct RPCServer {
    pub handlers: Arc<HashMap<String, Box<dyn RpcHandler>>>,
    pub async_handlers: Arc<HashMap<String, Box<dyn AsyncRpcHandler>>>,
}

impl RPCServer {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(HashMap::new()),
            async_handlers: Arc::new(HashMap::new()),
        }
    }

    /// Register a handler for a specific command
    pub fn register<T, F>(mut self, func: F) -> Result<Self>
    where
        T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn + 'static,
        T::Return: Serialize + ToRPCType + 'static,
        F: RpcFunc<T, T::Return>,
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

    /// Register an async handler for a specific command
    pub fn register_async<T, F>(mut self, func: F) -> Result<Self>
    where
        T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn + 'static,
        T::Return: Serialize + ToRPCType + 'static,
        F: AsyncRpcFunc<T, T::Return>,
    {
        // has to be RPCType::RPC
        let command = match <T as ToRPCType>::to_rpc_type() {
            RPCType::RPC(s) => s,
            _ => anyhow::bail!("Handler function argument has to be RPCType::RPC"),
        };

        let handler = AsyncHandler {
            func,
            _phantom: std::marker::PhantomData,
        };

        Arc::get_mut(&mut self.async_handlers)
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

    /// Dispatch a raw Lisp RPC string to the appropriate handler asynchronously
    pub async fn handle_async(&self, raw_data: &str) -> Result<String> {
        // 1. Extract the command name from the Lisp string (e.g., "(command-name ...)")
        let command =
            extract_command_name(raw_data).ok_or_else(|| anyhow!("Invalid RPC format"))?;

        // 2. Caution: Find the registered handler (check sync first)
        if let Some(handler) = self.handlers.get(&command) {
            let resp_obj = handler.handle(raw_data)?;
            return resp_obj.serialize_lisp();
        }

        // 3. Find the registered async handler
        if let Some(handler) = self.async_handlers.get(&command) {
            let resp_obj = handler.handle(raw_data).await?;
            return resp_obj.serialize_lisp();
        }

        anyhow::bail!("Unknown command: {}", command)
    }
}

/// Helper to get the first symbol from "(symbol ...)"
fn extract_command_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches('(').trim_end_matches(')');
    trimmed.split_whitespace().next().map(|s| s.to_string())
}
