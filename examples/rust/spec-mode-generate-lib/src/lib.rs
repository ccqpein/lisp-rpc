
#[derive(Debug)]
pub struct LanguagePerfer {
    lang: String,
}

impl ToRPCData for LanguagePerfer {
    fn to_rpc(&self) -> String {
        format!(
            "(language-perfer :lang {})",
            self.lang.to_rpc()
        )
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
            "(book-info :lang {} :title {} :version {} :id {})",
            self.lang.to_rpc(),
            self.title.to_rpc(),
            self.version.to_rpc(),
            self.id.to_rpc()
        )
    }
}
#[derive(Debug)]
pub struct GetBookLang {
    lang: String,
    encoding: i64,
}

impl ToRPCData for GetBookLang {
    fn to_rpc(&self) -> String {
        format!(
            "'(:lang {} :encoding {})",
            self.lang.to_rpc(),
            self.encoding.to_rpc()
        )
    }
}

#[derive(Debug)]
pub struct GetBook {
    title: String,
    vesion: String,
    lang: GetBookLang,
}

impl ToRPCData for GetBook {
    fn to_rpc(&self) -> String {
        format!(
            "(get-book :title {} :vesion {} :lang {})",
            self.title.to_rpc(),
            self.vesion.to_rpc(),
            self.lang.to_rpc()
        )
    }
}
#[derive(Debug)]
pub struct Authors {
    names: Vec<String>,
}

impl ToRPCData for Authors {
    fn to_rpc(&self) -> String {
        format!(
            "(authors :names {})",
            self.names.to_rpc()
        )
    }
}
