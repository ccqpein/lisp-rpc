pub use lisp_rpc_rust_serializer::lisp_rpc_to_str;

pub mod server;
pub use server::*;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_types_impl_to_rpc() {
        let val_bool = true;
        let val_char = 'a';
        let val_i8 = 1i8;
        let val_i16 = 2i16;
        let val_i32 = 3i32;
        let val_i64 = 4i64;
        let val_isize = 6isize;
        let val_u8 = 7u8;
        let val_u16 = 8u16;
        let val_u32 = 9u32;
        let val_u64 = 10u64;
        let val_usize = 12usize;
        let val_f32 = 13.0f32;
        let val_f64 = 14.0f64;
        let val_str = "hello";
        let val_string = "world".to_string();
        let val_unit = ();

        assert!(val_bool.serialize_lisp().is_ok());
        assert!(val_char.serialize_lisp().is_ok());
        assert!(val_i8.serialize_lisp().is_ok());
        assert!(val_i16.serialize_lisp().is_ok());
        assert!(val_i32.serialize_lisp().is_ok());
        assert!(val_i64.serialize_lisp().is_ok());
        assert!(val_isize.serialize_lisp().is_ok());
        assert!(val_u8.serialize_lisp().is_ok());
        assert!(val_u16.serialize_lisp().is_ok());
        assert!(val_u32.serialize_lisp().is_ok());
        assert!(val_u64.serialize_lisp().is_ok());
        assert!(val_usize.serialize_lisp().is_ok());
        assert!(val_f32.serialize_lisp().is_ok());
        assert!(val_f64.serialize_lisp().is_ok());
        assert!(val_str.serialize_lisp().is_ok());
        assert!(val_string.serialize_lisp().is_ok());
        assert!(val_unit.serialize_lisp().is_ok());
    }
}
