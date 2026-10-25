//! Core RPC dispatch engine for registering synchronous and asynchronous handlers.

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

/// Synchronous RPC handler function trait.
pub trait RpcFunc<T, R>: Send + Sync + 'static
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn<Return = R> + 'static,
    R: Serialize + ToRPCType + 'static,
{
    /// Executes the RPC handler with the decoded request.
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

/// Asynchronous RPC handler function trait.
pub trait AsyncRpcFunc<T, R>: Send + Sync + 'static
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + ToRPCReturn<Return = R> + 'static,
    R: Serialize + ToRPCType + 'static,
{
    /// Future type returned by the async handler.
    type Fut: Future<Output = Result<R>> + Send + 'static;
    /// Executes the asynchronous RPC handler with the decoded request.
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

/// Type-erased synchronous RPC handler.
pub trait RpcHandler: Send + Sync {
    /// Dispatches the raw S-expression string to the inner handler.
    fn handle(&self, raw_data: &str) -> Result<Box<dyn ToRPCType>>;

    /// Dispatches the raw S-expression string and serializes the response directly without boxing.
    fn handle_to_str(&self, raw_data: &str) -> Result<String> {
        self.handle(raw_data)?.serialize_lisp()
    }
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

    fn handle_to_str(&self, raw_data: &str) -> Result<String> {
        let req: T =
            lisp_rpc_from_str(raw_data).map_err(|e| anyhow!("Deserialization failed: {}", e))?;
        let resp = self.func.call(req)?;
        resp.serialize_lisp()
    }
}

/// Type-erased asynchronous RPC handler.
pub trait AsyncRpcHandler: Send + Sync {
    /// Dispatches the raw S-expression string to the inner async handler.
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

/// Dispatch engine that manages handler registries and routes incoming Lisp-RPC strings.
#[derive(Clone)]
pub struct RPCServer {
    /// Registered synchronous RPC handlers mapped by command name.
    pub handlers: Arc<HashMap<String, Box<dyn RpcHandler>>>,
    /// Registered asynchronous RPC handlers mapped by command name.
    pub async_handlers: Arc<HashMap<String, Box<dyn AsyncRpcHandler>>>,
}

impl RPCServer {
    /// Creates a new empty [`RPCServer`] instance.
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(HashMap::new()),
            async_handlers: Arc::new(HashMap::new()),
        }
    }

    /// Registers a synchronous handler for an RPC command type `T`.
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

    /// Registers an asynchronous handler for an RPC command type `T`.
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

    /// Dispatches a raw Lisp-RPC string synchronously and returns the serialized response.
    pub fn handle(&self, raw_data: &str) -> Result<String> {
        // 1. Extract the command name from the Lisp string (e.g., "(command-name ...)")
        let command =
            extract_command_name(raw_data).ok_or_else(|| anyhow!("Invalid RPC format"))?;

        // 2. Find the registered handler
        let handler = self
            .handlers
            .get(command)
            .ok_or_else(|| anyhow!("Unknown command: {}", command))?;

        // 3. Execute the handler and serialize the response directly without boxing
        handler.handle_to_str(raw_data)
    }

    /// Dispatches a raw Lisp-RPC string asynchronously and returns the serialized response.
    pub async fn handle_async(&self, raw_data: &str) -> Result<String> {
        // 1. Extract the command name from the Lisp string (e.g., "(command-name ...)")
        let command =
            extract_command_name(raw_data).ok_or_else(|| anyhow!("Invalid RPC format"))?;

        // 2. Caution: Find the registered handler (check sync first)
        if let Some(handler) = self.handlers.get(command) {
            return handler.handle_to_str(raw_data);
        }

        // 3. Find the registered async handler
        if let Some(handler) = self.async_handlers.get(command) {
            let resp_obj = handler.handle(raw_data).await?;
            return resp_obj.serialize_lisp();
        }

        anyhow::bail!("Unknown command: {}", command)
    }
}

/// Helper to get the first symbol slice from "(symbol ...)" without allocating
#[inline]
fn extract_command_name(raw: &str) -> Option<&str> {
    let trimmed = raw.trim().trim_start_matches('(').trim_end_matches(')');
    trimmed.split_whitespace().next()
}
