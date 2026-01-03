// let me assume I have this struct have been generate by generater
use super::*;

#[derive(Debug)]
pub struct LanguagePerfer {
    lang: String,
}

impl ToRPCData for LanguagePerfer {
    fn to_rpc(&self) -> String {
        format!("(language-perfer :lang {})", self.lang.to_rpc())
    }
}

#[derive(Debug)]
pub struct BookInfo {
    lang: LanguagePerfer,
    title: String,
    version: String,
    id: String,
}

impl ToRPCData for BookInfo {
    fn to_rpc(&self) -> String {
        format!(
            "(book-info :id {} :title {} :version {} :lang {})",
            self.id.to_rpc(),
            self.title.to_rpc(),
            self.version.to_rpc(),
            self.lang.to_rpc()
        )
    }
}

// rpc + keyword name
pub struct GetBookLang {
    lang: String,
    encoding: i64,
}

impl ToRPCData for GetBookLang {
    fn to_rpc(&self) -> String {
        format!(
            "'(:lang {} :encoding {})",
            self.lang.to_rpc(),
            self.encoding.to_rpc(),
        )
    }
}

pub struct GetBook {
    title: String,
    version: String,
    lang: GetBookLang,
}

impl ToRPCData for GetBook {
    fn to_rpc(&self) -> String {
        format!(
            "(get-book :title {} :version {} :lang {})",
            self.title.to_rpc(),
            self.version.to_rpc(),
            self.lang.to_rpc()
        )
    }
}

// test below for making sure
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_book_to_rpc() {
        let gb = GetBook {
            title: "hello world".to_string(),
            version: "1984".to_string(),
            lang: GetBookLang {
                lang: "english".to_string(),
                encoding: 11,
            },
        };

        assert_eq!(
            gb.to_rpc(),
            r#"(get-book :title "hello world" :version "1984" :lang '(:lang "english" :encoding 11))"#
        )
    }

    #[test]
    fn test_book_info_to_rpc() {
        let bi = BookInfo {
            lang: LanguagePerfer {
                lang: "english".to_string(),
            },
            title: "hello world".to_string(),
            version: "1984".to_string(),
            id: "123".to_string(),
        };
        assert_eq!(
            bi.to_rpc(),
            r#"(book-info :id "123" :title "hello world" :version "1984" :lang (language-perfer :lang "english"))"#
        )
    }
}
