use lisp_rpc_rust_serializer::lisp_rpc_to_str;

pub mod rpc_libs;
pub mod rpc_server;

#[derive(Debug, Clone, PartialEq)]
pub enum RPCType {
    Msg(String),
    RPC(String),
    Map,
    List,

    /// default value
    V,
}

/// need impl for struct
pub trait ToRPCType: Send + Sync {
    fn to_rpc_type() -> RPCType
    where
        Self: Sized,
    {
        RPCType::V
    }

    /// Object-safe serialization method
    fn serialize_lisp(&self) -> anyhow::Result<String>;
}

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

impl_to_rpc_basic!(String, &str, ());
