// let me assume I have this struct have been generate by generater
use super::*;
use lisp_rpc_rust_generator_macro::*;

#[derive(Debug, RPCData)]
pub struct LanguagePerfer {
    lang: String,
}

impl ToRPCType for LanguagePerfer {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::Msg("language-perfer".to_string())
    }
}

#[derive(Debug, RPCData)]
pub struct BookInfo {
    id: String,
    title: String,
    version: String,
    lang: LanguagePerfer,
}

impl ToRPCType for BookInfo {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::Msg("book-info".to_string())
    }
}

// rpc + keyword name
#[derive(RPCData)]
pub struct GetBookLang {
    lang: String,
    encoding: i64,
}

impl ToRPCType for GetBookLang {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::Map
    }
}

#[derive(RPCData)]
pub struct GetBook {
    title: String,
    version: String,
    lang: GetBookLang,
}

impl ToRPCType for GetBook {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::RPC("get-book".to_string())
    }
}

// test below for making sure
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_book_rpc_data() {
        let gb = GetBook {
            title: "hello world".to_string(),
            version: "1984".to_string(),
            lang: GetBookLang {
                lang: "english".to_string(),
                encoding: 11,
            },
        };

        assert_eq!(
            gb.rpc_data(),
            r#"(get-book :title "hello world" :version "1984" :lang '(:lang "english" :encoding 11))"#
        )
    }

    #[test]
    fn test_book_info_rpc_data() {
        let bi = BookInfo {
            id: "123".to_string(),
            title: "hello world".to_string(),
            version: "1984".to_string(),
            lang: LanguagePerfer {
                lang: "english".to_string(),
            },
        };
        assert_eq!(
            bi.rpc_data(),
            r#"(book-info :id "123" :title "hello world" :version "1984" :lang (language-perfer :lang "english"))"#
        )
    }
}
