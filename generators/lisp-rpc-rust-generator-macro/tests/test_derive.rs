use lisp_rpc_rust_generator_macro::RPCData;

// Mock traits
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

trait RPCData: ToRPCType {
    fn rpc_raw_data(&self) -> String;
    fn rpc_data(&self) -> String {
        match self.to_rpc_type() {
            RPCType::Msg(x) | RPCType::RPC(x) => format!("({} {})", x, self.rpc_data()),
            RPCType::Map | RPCType::List => "'(".to_string() + &self.rpc_data() + ")",
            RPCType::V => self.rpc_raw_data(),
        }
    }
}

// Mock impls
impl ToRPCType for String {}
impl RPCData for String {
    fn rpc_raw_data(&self) -> String {
        format!("\"{}\"", self)
    }
}

struct LanguagePerfer;
impl ToRPCType for LanguagePerfer {}
impl RPCData for LanguagePerfer {
    fn rpc_raw_data(&self) -> String {
        "lang-val".to_string()
    }
}

#[derive(RPCData)]
struct BookInfo {
    lang: LanguagePerfer,
    title: String,
    version: String,
    id: String,
}
// We need to implement ToRPCType manually because the derive only handles RPCData
impl ToRPCType for BookInfo {}

#[test]
fn test_book_info_rpc_data() {
    let book = BookInfo {
        lang: LanguagePerfer,
        title: "The Book".to_string(),
        version: "1.0".to_string(),
        id: "123".to_string(),
    };

    // Expected: :lang lang-val :title "The Book" :version "1.0" :id "123"
    // Note: The macro generates fields in definition order.
    let expected = ":lang lang-val :title \"The Book\" :version \"1.0\" :id \"123\"";
    let actual = book.rpc_raw_data();

    assert_eq!(actual, expected);
}
