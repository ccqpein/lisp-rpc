#[derive(Debug)]
pub enum RPCType {
    Msg(String),
    RPC(String),
    Map,
    List,

    /// default value
    V,
}

/// need impl for struct
pub trait ToRPCType {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::V
    }
}
