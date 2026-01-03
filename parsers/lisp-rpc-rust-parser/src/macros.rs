#[macro_export]
macro_rules! impl_into_data_for_numbers {
    ($($type:ty),*) => {
        $(
            impl IntoData for $type {
                fn into_rpc_data(&self) -> Data {
                    Data::Value(TypeValue::Number(*self as i64))
                }
            }
        )*
    };
}
