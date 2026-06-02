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

/// A trait that captures the relationship between a request type T and its async response.
pub trait AsyncRpcFunc<T>: Send + Sync + 'static {
    type Resp: Serialize + ToRPCType + 'static;
    type Fut: Future<Output = Result<Self::Resp>> + Send + 'static;
    fn call(&self, req: T) -> Self::Fut;
}

impl<T, R, F, Fut> AsyncRpcFunc<T> for F
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + 'static,
    R: Serialize + ToRPCType + 'static,
    Fut: Future<Output = Result<R>> + Send + 'static,
    F: Fn(T) -> Fut + Send + Sync + 'static,
{
    type Resp = R;
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

/// The type-erased async handler trait
pub trait AsyncRpcHandler: Send + Sync {
    fn handle(&self, raw_data: &str) -> Pin<Box<dyn Future<Output = Result<Box<dyn ToRPCType>>> + Send>>;
}

/// A concrete async handler that knows its own request type T
struct AsyncHandler<T, F> {
    func: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> AsyncRpcHandler for AsyncHandler<T, F>
where
    T: DeserializeOwned + Debug + Send + Sync + ToRPCType + 'static,
    F: AsyncRpcFunc<T>,
{
    fn handle(&self, raw_data: &str) -> Pin<Box<dyn Future<Output = Result<Box<dyn ToRPCType>>> + Send>> {
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

    /// Register an async handler for a specific command
    pub fn register_async<T, F>(mut self, func: F) -> Result<Self>
    where
        T: DeserializeOwned + Debug + Send + Sync + ToRPCType + 'static,
        F: AsyncRpcFunc<T>,
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

        // 2. Find the registered handler (check sync first)
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
    let trimmed = raw.trim().trim_start_matches('(');
    trimmed.split_whitespace().next().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    #[serde(rename = "dummy")]
    struct DummyReq {
        val: String,
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    #[serde(rename = "dummy-async")]
    struct DummyAsyncReq {
        val: String,
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct DummyResp {
        res: String,
    }

    impl_to_rpc!(DummyReq, RPCType::RPC("dummy".to_string()));
    impl_to_rpc!(DummyAsyncReq, RPCType::RPC("dummy-async".to_string()));
    impl_to_rpc!(DummyResp, RPCType::V);

    #[actix_web::test]
    async fn test_async_register_and_handle() {
        let server = RPCServer::new()
            .register(|req: DummyReq| {
                Ok(DummyResp {
                    res: format!("sync-{}", req.val),
                })
            })
            .unwrap()
            .register_async(|req: DummyAsyncReq| async move {
                Ok(DummyResp {
                    res: format!("async-{}", req.val),
                })
            })
            .unwrap();

        // 1. Test sync handler via sync dispatch
        let sync_req = lisp_rpc_rust_serializer::lisp_rpc_to_str(&DummyReq { val: "test".to_string() }).unwrap();
        let sync_res = server.handle(&sync_req).unwrap();
        assert!(sync_res.contains("sync-test"));

        // 2. Test async handler via handle_async
        let async_req = lisp_rpc_rust_serializer::lisp_rpc_to_str(&DummyAsyncReq { val: "async-test".to_string() }).unwrap();
        let async_res = server.handle_async(&async_req).await.unwrap();
        assert!(async_res.contains("async-async-test"));
    }
}
