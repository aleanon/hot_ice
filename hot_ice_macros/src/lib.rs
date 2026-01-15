mod hot_fn;
mod hot_state;


#[proc_macro_attribute]
pub fn hot_state(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    crate::hot_state::hot_state(_attr, item)
}

#[proc_macro_attribute]
pub fn hot_fn(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    crate::hot_fn::hot_fn(attr, item)
}
