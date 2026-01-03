extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, Ident, Type, parse_macro_input};

/// A procedural macro that can be derived on a struct to provide a method
/// that lists its fields and their types.
///
/// Example:
/// ```
/// use my_derive_macro::PrintFields;
///
/// #[derive(PrintFields)]
/// pub struct BookInfo {
///     lang: String,
///     title: String,
///     version: String,
///     id: String,
/// }
///
/// fn main() {
///     BookInfo::print_fields();
/// }
/// ```
#[proc_macro_derive(LispRPCRaw)]
pub fn print_fields_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data_struct) => &data_struct.fields,
        _ => panic!("PrintFields can only be derived on structs"),
    };

    let field_printers = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().expect("Expected named field");
        let field_type = &field.ty;

        // Convert the type to a string for printing.
        // This uses `.to_token_stream().to_string()`
        // because `Type` itself doesn't directly give a string name.
        let field_type_str = field_type.to_token_stream().to_string();

        quote! {
            println!("  Field: {}, Type: {}", stringify!(#field_name), #field_type_str);
        }
    });

    let expanded = quote! {
        impl #impl_generics #struct_name #ty_generics #where_clause {
            pub fn print_fields() {
                println!("Struct: {}", stringify!(#struct_name));
                #(#field_printers)*
            }
        }
    };

    expanded.into()
}
