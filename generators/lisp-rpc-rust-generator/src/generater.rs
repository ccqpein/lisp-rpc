use super::*;
use serde::Serialize;
use tera::Context;

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub enum RPCDataType {
    Map,
    List,
    Msg,
    Rpc,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct GeneratedField {
    pub name: String,
    pub field_type: String,
    pub comment: Option<String>,
}

impl GeneratedField {
    pub fn new(name: String, field_type: String, comment: Option<String>) -> Result<Self> {
        const RESERVED_WORDS: &[&str] = &["type"];

        if RESERVED_WORDS.contains(&name.as_str()) {
            anyhow::bail!("Field name {} is reserved words", &name)
        }

        Ok(Self {
            name: name,
            field_type: field_type,
            comment,
        })
    }
}

/// the GeneratedStruct is the middle layer between render and rpc spec (msg and rpc)
/// def pkg is too simple, no need this
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct GeneratedStruct {
    pub name: String,
    pub fields: Vec<GeneratedField>,
    pub comment: Option<String>,

    /// the original data name
    /// for insert the impl block of gen_data
    data_name: String,

    /// different types have different data format
    /// this for detect which is which
    pub rpc_type: RPCDataType,
}

impl GeneratedStruct {
    pub fn new(
        data_name: &str,
        fields: Vec<GeneratedField>,
        comment: Option<String>,
        ty: RPCDataType,
    ) -> Self {
        Self {
            name: kebab_to_pascal_case(data_name),
            fields,
            comment,

            data_name: data_name.to_string(),

            rpc_type: ty,
        }
    }

    pub fn insert_template(&self, ctx: &mut Context) {
        ctx.insert("name", &self.name);
        ctx.insert("fields", &self.fields);

        match self.rpc_type {
            RPCDataType::Map => {
                ctx.insert("ty", "map");
            }
            RPCDataType::List => {
                ctx.insert("ty", "list");
            }
            RPCDataType::Msg => {
                ctx.insert("data_name", &self.data_name);
                ctx.insert("ty", "msg");
            }
            RPCDataType::Rpc => {
                ctx.insert("data_name", &self.data_name);
                ctx.insert("ty", "rpc");
            }
        }
    }

    pub fn gen_code_with_files(&self, template_files: &[impl AsRef<Path>]) -> Result<String> {
        let mut tera = Tera::default();
        let mut context = Context::new();

        let mut all_temps = vec![];
        for p in template_files {
            match p.as_ref().file_stem().map(|n| n.to_str()) {
                Some(n) => {
                    all_temps.push((p, n));
                }
                None => (),
            }
        }

        tera.add_template_files(all_temps)?;

        let mut result = String::new();

        self.insert_template(&mut context);
        result += &tera.render("def_struct.rs", &context)?;
        result += "\n\n";
        result += &tera.render("rpc_impl", &context)?;

        Ok(result)
    }

    pub fn gen_code_with_tera(&self, templates: &Tera) -> Result<String> {
        let mut context = Context::new();

        let mut result = String::new();
        self.insert_template(&mut context);
        result += &templates.render("def_struct.rs", &context)?;
        result += "\n\n";
        result += &templates.render("rpc_impl", &context)?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use tera::{Context, Tera};

    #[test]
    fn test_generate_field_reserved_words() -> Result<()> {
        assert!(GeneratedField::new("type".to_string(), "String".to_string(), None).is_err());
        Ok(())
    }

    #[test]
    fn test_generate_struct() {
        let temp = include_str!("../templates/def_struct.rs.template");
        let mut tera = Tera::default();
        let mut context = Context::new();

        //dbg!(temp);
        tera.add_raw_template("test", temp).unwrap();

        let s = GeneratedStruct {
            name: "name".to_string(),
            fields: vec![
                GeneratedField::new("a".to_string(), "String".to_string(), None).unwrap(),
                GeneratedField::new("a".to_string(), "i64".to_string(), None).unwrap(),
                GeneratedField::new("a".to_string(), "OtherType".to_string(), None).unwrap(),
            ],
            comment: None,
            data_name: "name".to_string(),
            rpc_type: RPCDataType::Msg,
        };

        context.insert("name", &s.name);
        context.insert("fields", &s.fields);
        context.insert("data_name", &s.data_name);
        //dbg!(tera.render("test", &context).unwrap());
        assert_eq!(
            tera.render("test", &context).unwrap(),
            r#"#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct name {
    pub a: String,
    pub a: i64,
    pub a: OtherType,
}"#
        );

        // empty fields
        let s = GeneratedStruct {
            name: "name".to_string(),
            fields: vec![],
            comment: None,
            data_name: "name".to_string(),
            rpc_type: RPCDataType::Msg,
        };

        context.insert("name", &s.name);
        context.insert("fields", &s.fields);
        context.insert("data_name", &s.data_name);
        //dbg!(tera.render("test", &context).unwrap());
        assert_eq!(
            tera.render("test", &context).unwrap(),
            r#"#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct name {
}"#
        );
    }

    #[test]
    fn test_generate_trait() {
        let temp = include_str!("../templates/rpc_impl.template");
        let mut tera = Tera::default();
        let mut context = Context::new();

        //dbg!(temp);
        tera.add_raw_template("test", temp).unwrap();

        let s = GeneratedStruct {
            name: "name".to_string(),
            fields: vec![
                GeneratedField::new("a".to_string(), "String".to_string(), None).unwrap(),
                GeneratedField::new("a".to_string(), "i64".to_string(), None).unwrap(),
            ],
            comment: None,
            data_name: "name".to_string(),
            rpc_type: RPCDataType::Msg,
        };

        context.insert("name", &s.name);
        context.insert("fields", &s.fields);
        context.insert("data_name", &s.data_name);
        context.insert("ty", "msg");
        //dbg!(tera.render("test", &context).unwrap());
        assert_eq!(
            tera.render("test", &context).unwrap(),
            r#"impl_to_rpc!(name, RPCType::Msg("name".to_string()));"#
        );

        //
        let mut context = Context::new();
        context.insert("name", &s.name);
        context.insert("fields", &s.fields);
        context.insert("ty", "map");
        assert_eq!(
            tera.render("test", &context).unwrap(),
            r#"impl_to_rpc!(name, RPCType::Map);"#
        );
    }

    #[test]
    fn test_generate_init_func() {
        let temp = include_str!("../templates/init.template");
        let mut tera = Tera::default();
        let mut context = Context::new();

        tera.add_raw_template("init", temp).unwrap();

        context.insert("map_types", &["A", "B"]);
        assert_eq!(
            tera.render("init", &context).unwrap(),
            r#"pub fn init() {
    register_global_map_type("A");
    register_global_map_type("B");
}
"#
        );
    }
}
