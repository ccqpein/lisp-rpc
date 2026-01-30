mod rpc_libs;

// macro_rules! impl_to_rpc_type {
//     ($($type:ty),*) => {
//         $(
//             impl ToRPCType for $type {
//                 fn to_rpc_type(&self) -> RPCType {
//                     RPCType::V
//                 }
//             }
//         )*
//     };
// }

// #[derive(Debug)]
// enum RPCType {
//     Msg(String),
//     RPC(String),
//     Map,
//     List,

//     /// default value
//     V,
// }

// trait ToRPCType {
//     fn to_rpc_type(&self) -> RPCType {
//         RPCType::V
//     }
// }

// trait ToRPCData: ToRPCType {
//     fn to_rpc_inner(&self) -> String;
//     fn to_rpc(&self) -> String {
//         match self.to_rpc_type() {
//             RPCType::Msg(x) | RPCType::RPC(x) => format!("({} {})", x, self.to_rpc_inner()),
//             RPCType::Map | RPCType::List => "'(".to_string() + &self.to_rpc_inner() + ")",
//             RPCType::V => self.to_rpc_inner(),
//         }
//     }
// }

// impl_to_rpc_type!(String, i64);

// impl<T: ToRPCType> ToRPCType for Vec<T> {
//     fn to_rpc_type(&self) -> RPCType {
//         RPCType::List
//     }
// }

// impl ToRPCData for String {
//     // fn to_rpc(&self) -> String {
//     //     format!("\"{}\"", self.to_string())
//     // }

//     fn to_rpc_inner(&self) -> String {
//         format!("\"{}\"", self.to_string())
//     }
// }

// impl ToRPCData for i64 {
//     // fn to_rpc(&self) -> String {
//     //     self.to_string()
//     // }

//     fn to_rpc_inner(&self) -> String {
//         self.to_string()
//     }
// }

// impl<T: ToRPCData> ToRPCData for Vec<T> {
//     fn to_rpc_inner(&self) -> String {
//         self.iter()
//             .map(|e| e.to_rpc())
//             .collect::<Vec<_>>()
//             .join(" ")
//     }

//     // fn to_rpc(&self) -> String {
//     //     "'(".to_string()
//     //         + &self
//     //             .iter()
//     //             .map(|e| e.to_rpc())
//     //             .collect::<Vec<_>>()
//     //             .join(" ")
//     //         + ")"
//     // }
// }

// trait FromRPCData {
//     fn from_rpc(&self) -> String;
// }

//trait RPCMapData: {}

////////////////////////////////

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

    //fn to_rpc_data_format(&self) ->
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

// fn to_rpc_data(a: &impl RPCData) -> String {
//     match a.to_rpc_type() {
//         RPCType::Msg(x) | RPCType::RPC(x) => format!("({} {})", x, a.rpc_data()),
//         RPCType::Map | RPCType::List => "'(".to_string() + &a.rpc_data() + ")",
//         RPCType::V => a.rpc_data(),
//     }
// }

macro_rules! impl_to_rpc_data_type {
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

impl_to_rpc_data_type!(String, i64);

impl RPCData for String {
    fn rpc_raw_data(&self) -> String {
        format!("\"{}\"", self.to_string())
    }
}

impl RPCData for i64 {
    fn rpc_raw_data(&self) -> String {
        self.to_string()
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
