pub mod rpc_libs;
pub mod rpc_server;

#[derive(Debug)]
enum RPCType {
    Msg(String),
    RPC(String),
    Map,
    List,

    /// default value
    V,
}

/// need impl for struct
trait ToRPCType {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::V
    }
}
