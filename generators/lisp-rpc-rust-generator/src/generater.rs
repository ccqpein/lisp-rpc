use super::*;
use serde::Serialize;
use tera::Context;

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub enum RPCDataType {
    Map,
    List,
    Data,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct GeneratedField {
    pub name: String,
    pub field_type: String,
    pub comment: Option<String>,

    /// the original keyword name
    /// for insert the impl block of gen_data
    key_name: String,
}

impl GeneratedField {
    pub fn new(key_name: &str, field_type: &str, comment: Option<String>) -> Self {
        Self {
            name: kebab_to_snake_case(key_name),
            field_type: type_translate(field_type),
            comment,

            key_name: key_name.to_string(),
        }
    }
}

/// the GeneratedStruct is the middle layer between render and rpc spec (msg and rpc)
/// def pkg is too simple, no need this
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct GeneratedStruct {
    pub name: String,
    pub derived_traits: Option<Vec<String>>,
    pub fields: Vec<GeneratedField>,
    pub comment: Option<String>,

    /// the original data name
    /// for insert the impl block of gen_data
    data_name: String,

    /// different types have different data format
    /// this for detect which is which
    rpc_type: RPCDataType,
}

impl GeneratedStruct {
    pub fn new(
        data_name: &str,
        derived_traits: Option<Vec<String>>,
        fields: Vec<GeneratedField>,
        comment: Option<String>,
        ty: RPCDataType,
    ) -> Self {
        Self {
            name: kebab_to_pascal_case(data_name),
            derived_traits,
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
            RPCDataType::Data => {
                ctx.insert("data_name", &self.data_name);
                ctx.insert("ty", "data");
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use tera::{Context, Tera};

    #[test]
    fn test_generate_struct() {
        let temp = include_str!("../templates/def_struct.rs.template");
        let mut tera = Tera::default();
        let mut context = Context::new();

        //dbg!(temp);
        tera.add_raw_template("test", temp).unwrap();

        let s = GeneratedStruct {
            name: "name".to_string(),
            derived_traits: None,
            fields: vec![
                GeneratedField::new("a", "string", None),
                GeneratedField::new("a", "number", None),
            ],
            comment: None,
            data_name: "name".to_string(),
            rpc_type: RPCDataType::Data,
        };

        context.insert("name", &s.name);
        context.insert("fields", &s.fields);
        context.insert("data_name", &s.data_name);
        //dbg!(tera.render("test", &context).unwrap());
        assert_eq!(
            tera.render("test", &context).unwrap(),
            r#"#[derive(Debug)]
pub struct name {
    a: String,
    a: i64,
}"#
        );

        // empty fields
        let s = GeneratedStruct {
            name: "name".to_string(),
            derived_traits: None,
            fields: vec![],
            comment: None,
            data_name: "name".to_string(),
            rpc_type: RPCDataType::Data,
        };

        context.insert("name", &s.name);
        context.insert("fields", &s.fields);
        context.insert("data_name", &s.data_name);
        //dbg!(tera.render("test", &context).unwrap());
        assert_eq!(
            tera.render("test", &context).unwrap(),
            r#"#[derive(Debug)]
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
            derived_traits: None,
            fields: vec![
                GeneratedField::new("a", "string", None),
                GeneratedField::new("a", "number", None),
            ],
            comment: None,
            data_name: "name".to_string(),
            rpc_type: RPCDataType::Data,
        };

        context.insert("name", &s.name);
        context.insert("fields", &s.fields);
        context.insert("data_name", &s.data_name);
        context.insert("ty", "data");
        //dbg!(tera.render("test", &context).unwrap());
        assert_eq!(
            tera.render("test", &context).unwrap(),
            r#"impl ToRPCData for name {
    fn to_rpc(&self) -> String {
        format!(
            "(name :a {} :a {})",
            self.a.to_rpc(),
            self.a.to_rpc()
        )
    }
}"#
        );

        //
        let mut context = Context::new();
        context.insert("name", &s.name);
        context.insert("fields", &s.fields);
        context.insert("ty", "map");
        assert_eq!(
            tera.render("test", &context).unwrap(),
            r#"impl ToRPCData for name {
    fn to_rpc(&self) -> String {
        format!(
            "'(:a {} :a {})",
            self.a.to_rpc(),
            self.a.to_rpc()
        )
    }
}"#
        );
    }
}
