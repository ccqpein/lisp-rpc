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
