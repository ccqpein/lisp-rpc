//! Server implementation for Lisp-RPC, providing request dispatching and RPC type traits.

pub use lisp_rpc_rust_serializer::lisp_rpc_to_str;

pub mod server;
pub use server::*;

/// Classification of RPC data structures and commands.
#[derive(Debug, Clone, PartialEq)]
pub enum RPCType {
    /// Named message type.
    Msg(String),
    /// Named RPC command.
    RPC(String),
    /// Quoted anonymous map.
    Map,
    /// Quoted list sequence.
    List,

    /// Primitive atom value.
    V,
}

/// Trait for types that can be classified into an [`RPCType`] and serialized to Lisp-RPC strings.
pub trait ToRPCType: Send + Sync {
    /// Returns the [`RPCType`] associated with this type.
    fn to_rpc_type() -> RPCType
    where
        Self: Sized,
    {
        RPCType::V
    }

    /// Serializes `self` into a Lisp-RPC S-expression string.
    fn serialize_lisp(&self) -> anyhow::Result<String>;
}

/// Trait associating an RPC request type with its corresponding return type.
pub trait ToRPCReturn: Send + Sync + ToRPCType {
    /// The response type returned by the RPC handler.
    type Return: Send + Sync + ToRPCType;
}

/// Implements [`ToRPCType`] for a type with a specific [`RPCType`].
#[macro_export]
macro_rules! impl_to_rpc {
    ($t:ty, $rpc:expr) => {
        impl ToRPCType for $t {
            fn to_rpc_type() -> RPCType {
                $rpc
            }
            fn serialize_lisp(&self) -> anyhow::Result<String> {
                lisp_rpc_to_str(self).map_err(|e| anyhow::anyhow!(e))
            }
        }
    };
}

/// Implements [`ToRPCReturn`] linking a request type to its return type.
#[macro_export]
macro_rules! impl_to_rpc_return {
    ($t:ty, $r:ty) => {
        impl ToRPCReturn for $t {
            type Return = $r;
        }
    };
}

macro_rules! impl_to_rpc_basic {
    ($($t:ty),*) => {
        $(
            impl ToRPCType for $t {
                fn serialize_lisp(&self) -> anyhow::Result<String> {
                    lisp_rpc_to_str(self).map_err(|e| anyhow::anyhow!(e))
                }
            }
        )*
    };
}

impl_to_rpc_basic!(
    bool,
    char,
    i8,
    i16,
    i32,
    i64,
    isize,
    u8,
    u16,
    u32,
    u64,
    usize,
    f32,
    f64,
    String,
    &str,
    ()
);
