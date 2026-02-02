mod rpc_libs;

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

/// should be derive to struct
trait RPCData: ToRPCType {
    fn rpc_raw_data(&self) -> String;
    fn rpc_data(&self) -> String {
        match self.to_rpc_type() {
            RPCType::Msg(x) | RPCType::RPC(x) => format!("({} {})", x, self.rpc_raw_data()),
            RPCType::Map | RPCType::List => "'(".to_string() + &self.rpc_raw_data() + ")",
            RPCType::V => self.rpc_raw_data(),
        }
    }
}

macro_rules! impl_to_rpc_data_type_v {
    ($($type:ty),*) => {
        $(
            impl ToRPCType for $type {
                fn to_rpc_type(&self) -> RPCType {
                    RPCType::V
                }
            }
        )*
    };
}

macro_rules! impl_to_rpc_data_to_string {
    ($($type:ty),*) => {
        $(
            impl RPCData for $type {
                fn rpc_raw_data(&self) -> String {
                    self.to_string()
                }
            }
        )*
    };
}

impl_to_rpc_data_type_v!(String, i64);
impl_to_rpc_data_to_string!(i64);

impl RPCData for String {
    fn rpc_raw_data(&self) -> String {
        format!("\"{}\"", self.to_string())
    }
}

impl<T: RPCData> ToRPCType for Vec<T> {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::List
    }
}

impl<T: RPCData> RPCData for Vec<T> {
    fn rpc_raw_data(&self) -> String {
        self.iter()
            .map(|e| e.rpc_data())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
