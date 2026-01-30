use lisp_rpc_rust_generator_macro::ToRPCData;

trait ToRPCData {
    fn to_rpc(&self) -> String;
}

// Mock impl for String
impl ToRPCData for String {
    fn to_rpc(&self) -> String {
        format!("\"{}\"", self)
    }
}

struct LanguagePerfer;
impl ToRPCData for LanguagePerfer {
    fn to_rpc(&self) -> String {
        "lang-val".to_string()
    }
}

#[derive(ToRPCData)]
struct BookInfo {
    lang: LanguagePerfer,
    title: String,
    version: String,
    id: String,
}

#[test]
fn test_book_info_to_rpc() {
    let book = BookInfo {
        lang: LanguagePerfer,
        title: "The Book".to_string(),
        version: "1.0".to_string(),
        id: "123".to_string(),
    };

    // Note: The macro generates fields in the order they are defined in the struct.
    // Struct order: lang, title, version, id.
    // Expected output: (book-info :lang lang-val :title "The Book" :version "1.0" :id "123")

    let expected = "(book-info :lang lang-val :title \"The Book\" :version \"1.0\" :id \"123\")";
    let actual = book.to_rpc();

    assert_eq!(actual, expected);
}
