use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Ident, ItemFn};

/// Attribute macro that transforms an update function to handle DynMessage conversion.
///
/// **Mark:** If you change the name of your function, you must recompile
///
/// Takes a function with signature:
/// ```ignore
/// fn my_update_logic(&self, message: Message) -> Task<Message>
/// ```
///
/// And transforms it into:
/// ```ignore
/// fn my_update_logic(&self, message: hot_ice::HotMessage) -> hot_ice::runtime::Task<hot_ice::HotMessage> {
///     let message = message.into_message().unwrap()
///
///     Ok(Self::my_update_logic_inner(self, message)
///         .map(hot_ice::DynMessage::into_hot_message))
/// }
///
/// fn my_update_logic_inner(&self, message: Message) -> Task<Message> {
///     // Your logic here
/// }
/// ```
#[proc_macro_attribute]
pub fn hot_update(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner", &input.sig.ident);
    let inner_fn_ident = Ident::new(&inner_fn_name, Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let expanded = quote! {
        #[no_mangle]
        #vis fn #original_fn_name(
            &mut self,
            message: hot_ice::HotMessage,
        ) -> hot_ice::HotTask<hot_ice::HotMessage> {
            let message = message.into_message().unwrap();

            Self::#inner_fn_ident(self, message)
                .map(hot_ice::DynMessage::into_hot_message)
        }

        #input
    };

    TokenStream::from(expanded)
}

/// Attribute macro that transforms a view function to handle HotMessage conversion.
///
/// **Mark:** If you change the name of your function, you must recompile
///
/// Takes a function with signature:
/// ```ignore
/// fn my_view(&self) -> Element<Message>
/// ```
///
/// And transforms it into:
/// ```ignore
/// fn my_view(&self) -> Element<hot_ice::HotMessage> {
///     Self::my_view_inner(&self)
///         .map(hot_ice::DynMessage::into_hot_message)
/// }
///
/// fn my_view_inner(&self) -> Element<Message> {
///     // Your view logic here
/// }
/// ```
#[proc_macro_attribute]
pub fn hot_view(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner", &input.sig.ident);
    let inner_fn_ident = Ident::new(&inner_fn_name, Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let expanded = quote! {
        #[no_mangle]
        #vis fn #original_fn_name(&self) -> hot_ice::HotElement<hot_ice::HotMessage> {
            Self::#inner_fn_ident(&self)
                .map(hot_ice::DynMessage::into_hot_message)
        }

        #input
    };

    TokenStream::from(expanded)
}
