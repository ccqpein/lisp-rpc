//! Helper macros for implementing [`crate::IntoData`] on primitive number types.

/// Implements [`crate::IntoData`] for integer primitive types.
#[macro_export]
macro_rules! impl_into_data_for_numbers_int {
    ($($type:ty),*) => {
        $(
            impl IntoData for $type {
                fn into_rpc_data(&self) -> Data {
                    Data::Value(TypeValue::Number(TypeValueNumber::Int(*self as i64)))
                }
            }
        )*
    };
}

/// Implements [`crate::IntoData`] for floating-point primitive types.
#[macro_export]
macro_rules! impl_into_data_for_numbers_float {
    ($($type:ty),*) => {
        $(
            impl IntoData for $type {
                fn into_rpc_data(&self) -> Data {
                    Data::Value(TypeValue::Number(TypeValueNumber::Float(*self as f64)))
                }
            }
        )*
    };
}
