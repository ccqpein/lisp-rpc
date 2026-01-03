mod rpc_libs;

// macro_rules! impl_to_rpc_data {
//     ($($type:ty),*) => {
//         $(
//             impl ToRPCData for $type {
//                 fn to_rpc(&self) -> String {
//                     format!("{}", self)
//                 }
//             }
//         )*
//     };
// }

#[derive(Debug)]
enum RPCTypes {
    Msg(String),
    RPC(String),
    Map,
    List,

    /// default value
    V,
}

trait ToRPCData {
    fn to_rpc(&self) -> String;

    /// get the type of this type
    fn get_type() -> RPCTypes {
        RPCTypes::V
    }
}

impl ToRPCData for String {
    fn to_rpc(&self) -> String {
        format!("\"{}\"", self.to_string())
    }
}

impl ToRPCData for i64 {
    fn to_rpc(&self) -> String {
        self.to_string()
    }
}

impl<T: ToRPCData> ToRPCData for Vec<T> {
    fn to_rpc(&self) -> String {
        "'(".to_string()
            + &self
                .iter()
                .map(|e| e.to_rpc())
                .collect::<Vec<_>>()
                .join(" ")
            + ")"
    }
}

trait FromRPCData {
    fn from_rpc(&self) -> String;
}
