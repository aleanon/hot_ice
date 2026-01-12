use hot_ice_common::{
    DESERIALIZE_STATE_FUNCTION_NAME, FREE_SERIALIZED_DATA_FUNCTION_NAME,
    SERIALIZE_STATE_FUNCTION_NAME,
};
use quote::{quote, quote_spanned};
use syn::{parse_macro_input, spanned::Spanned};

// Used to make sure the generated code does not conflict with user-defined functions
const INNER_FUNCTION_POSTFIX: &str = "sdlksldkdkslskfjei";

#[proc_macro_attribute]
pub fn hot_state(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut ast = parse_macro_input!(item as syn::DeriveInput);

    let mut has_deserialize = false;
    let mut has_serialize = false;
    let mut has_default = false;

    for attr in &ast.attrs {
        if attr.path().is_ident("derive") {
            let token_str = quote::ToTokens::to_token_stream(&attr.meta).to_string();
            if token_str.contains("Deserialize") {
                has_deserialize = true;
            }
            if token_str.contains("Serialize") {
                has_serialize = true;
            }
            if token_str.contains("Default") {
                has_default = true;
            }
        }
    }

    // Collect all missing derives
    let mut derives = vec![];

    if !has_serialize {
        derives.push(quote! { serde::Serialize });
    }
    if !has_deserialize {
        derives.push(quote! { serde::Deserialize });
    }
    if !has_default {
        derives.push(quote! { ::core::default::Default });
    }

    if !derives.is_empty() {
        let deser_attr: syn::Attribute = syn::parse_quote!(#[derive(#(#derives),*)]);
        ast.attrs.push(deser_attr);
    }

    let mut has_struct_default = false;
    for attr in &ast.attrs {
        if attr.path().is_ident("serde") {
            if quote::ToTokens::to_token_stream(&attr.meta)
                .to_string()
                .contains("default")
            {
                has_struct_default = true;
                break;
            }
        }
    }

    if !has_struct_default {
        let default_attr: syn::Attribute = syn::parse_quote!(#[serde(default)]);
        ast.attrs.push(default_attr);
    }

    let struct_name = &ast.ident;
    let serialize_state_ident = proc_macro2::Ident::new(
        SERIALIZE_STATE_FUNCTION_NAME,
        proc_macro2::Span::call_site(),
    );
    let deserialize_state_ident = proc_macro2::Ident::new(
        DESERIALIZE_STATE_FUNCTION_NAME,
        proc_macro2::Span::call_site(),
    );
    let free_serialized_data_ident = proc_macro2::Ident::new(
        FREE_SERIALIZED_DATA_FUNCTION_NAME,
        proc_macro2::Span::call_site(),
    );

    quote!(
        use hot_ice::*;

        #ast

        impl #struct_name {
            /// Serialize state and return raw pointer + length
            /// Caller must call free_serialized_data to free the memory
            #[unsafe(no_mangle)]
            pub fn #serialize_state_ident(
                state: &hot_ice::HotState,
                out_ptr: *mut *mut u8,
                out_len: *mut usize,
            ) -> Result<(), hot_ice::HotFunctionError> {
                let data = state.serialize_state::<Self>()?;

                let len = data.len();
                let mut boxed_slice = data.into_boxed_slice();
                let ptr = boxed_slice.as_mut_ptr();
                std::mem::forget(boxed_slice);

                unsafe {
                    *out_ptr = ptr;
                    *out_len = len;
                }

                Ok(())
            }

            #[unsafe(no_mangle)]
            pub fn #deserialize_state_ident(
                state: &mut hot_ice::HotState,
                data_ptr: *const u8,
                data_len: usize,
            ) -> Result<(), hot_ice::HotFunctionError> {
                let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
                state.deserialize_state::<Self>(data)
            }

            /// Free memory allocated by serialize_state
            #[unsafe(no_mangle)]
            pub fn #free_serialized_data_ident(ptr: *mut u8, len: usize) {
                if !ptr.is_null() && len > 0 {
                    unsafe {
                        let _ = Vec::from_raw_parts(ptr, len, len);
                        // Vec is dropped here, freeing the memory
                    }
                }
            }
        }
    )
    .into()
}

fn boot(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    // Extract the Message type from the return type
    let message_type = extract_message_type_from_return(&input.sig.output);

    let expanded = if hot_state {
        if let Some(msg_type) = message_type {
            // We have a Task<Message> in the return type - call function directly
            quote! {
                #vis fn #original_fn_name() -> (hot_ice::HotState, hot_ice::iced::Task<hot_ice::HotMessage>) {
                    let (app, task): (Self, hot_ice::iced::Task<#msg_type>) = Self::#inner_fn_ident();

                    (
                        hot_ice::HotState::new(app),
                        task.map(hot_ice::DynMessage::into_hot_message)
                    )
                }

                #input
            }
        } else {
            // No Task in return type - create empty task
            quote! {
                #vis fn #original_fn_name() -> (hot_ice::HotState, hot_ice::iced::Task<hot_ice::HotMessage>) {
                    let app = Self::#inner_fn_ident();

                    (
                        hot_ice::HotState::new(app),
                        hot_ice::iced::Task::none()
                    )
                }

                #input
            }
        }
    } else {
        if let Some(msg_type) = message_type {
            quote! {
                #vis fn #original_fn_name() -> (Self, hot_ice::iced::Task<hot_ice::HotMessage>) {
                    use hot_ice::IntoBoot;

                    let (app, task): (Self, hot_ice::iced::Task<#msg_type>) = Self::#inner_fn_ident();

                    (app, task.map(hot_ice::DynMessage::into_hot_message))
                }

                #input
            }
        } else {
            quote! {
                #vis fn #original_fn_name() -> (Self, hot_ice::iced::Task<hot_ice::HotMessage>) {
                    use hot_ice::IntoBoot;

                    let app = Self::#inner_fn_ident();

                    (app, hot_ice::iced::Task::none())
                }

                #input
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}

/// Extract the Message type from a return type like (Self, Task<Message>)
fn extract_message_type_from_return(output: &syn::ReturnType) -> Option<syn::Type> {
    if let syn::ReturnType::Type(_, ty) = output {
        if let syn::Type::Tuple(tuple) = &**ty {
            // Look for Task<T> in the tuple elements
            for elem in &tuple.elems {
                if let Some(msg_type) = extract_task_inner_type(elem) {
                    return Some(msg_type);
                }
            }
        }
    }
    None
}

/// Extract T from Task<T>
fn extract_task_inner_type(ty: &syn::Type) -> Option<syn::Type> {
    if let syn::Type::Path(type_path) = ty {
        // Check if the last segment is "Task"
        if let Some(last_seg) = type_path.path.segments.last() {
            if last_seg.ident == "Task" {
                // Extract the generic argument
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        return Some(inner_type.clone());
                    }
                }
            }
        }
    }
    None
}

fn update(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let expanded = if hot_state {
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(
                state: &mut hot_ice::HotState,
                message: hot_ice::HotMessage,
            ) -> Result<Task<hot_ice::HotMessage>, hot_ice::HotFunctionError> {
                let message = message
                    .into_message()
                    .map_err(|m| hot_ice::HotFunctionError::MessageDowncastError(format!("{:?}", m)))?;

                let task = Self::#inner_fn_ident(state.ref_mut_state(), message)
                    .map(hot_ice::DynMessage::into_hot_message);

                Ok(task)
            }
            #input
        }
    } else {
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(
                &mut self,
                message: hot_ice::HotMessage,
            ) -> Result<Task<hot_ice::HotMessage>, hot_ice::HotFunctionError> {
                let message = message.into_message()
                    .map_err(|message| hot_ice::HotFunctionError::MessageDowncastError(format!("{:?}", message)))?;

                let task = self.#inner_fn_ident(message)
                    .map(hot_ice::DynMessage::into_hot_message);

                Ok(task)
            }
            #input
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn view(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let expanded = if hot_state {
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(state: &hot_ice::HotState) -> Element<hot_ice::HotMessage> {
                Self::#inner_fn_ident(state.ref_state())
                    .map(hot_ice::DynMessage::into_hot_message)
            }

            #input
        }
    } else {
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(&self) -> Element<hot_ice::HotMessage> {
                self.#inner_fn_ident()
                    .map(hot_ice::DynMessage::into_hot_message)
            }

            #input
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn subscription(
    hot_state: bool,
    is_hot: bool,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let no_mangle_attr = if is_hot {
        quote! { #[unsafe(no_mangle)] }
    } else {
        quote! {}
    };

    let expanded = if hot_state {
        quote! {
            #no_mangle_attr
            #vis fn #original_fn_name(state: &hot_ice::HotState) -> hot_ice::iced::Subscription<hot_ice::HotMessage> {
                Self::#inner_fn_ident(state.ref_state())
                    .map(hot_ice::DynMessage::into_hot_message)
            }
            #input
        }
    } else {
        quote! {
            #no_mangle_attr
            #vis fn #original_fn_name(&self) -> hot_ice::iced::Subscription<hot_ice::HotMessage> {
                self.#inner_fn_ident()
                    .map(hot_ice::DynMessage::into_hot_message)
            }
            #input
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn theme(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;
    let return_type = &input.sig.output;

    let expanded = if hot_state {
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(state: &hot_ice::HotState) #return_type {
                Self::#inner_fn_ident(state.ref_state())
            }
            #input
        }
    } else {
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(&self) #return_type {
                self.#inner_fn_ident()
            }
            #input
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn style(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());

    let vis = &input.vis;
    let return_type = &input.sig.output;

    let mut args_no_receiver = Vec::new();
    let mut arg_names = Vec::new();
    for arg in input.sig.inputs.iter().skip(1) {
        args_no_receiver.push(arg);
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                arg_names.push(&pat_ident.ident);
            }
        }
    }

    input.sig.ident = inner_fn_ident.clone();

    let expanded = if hot_state {
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(state: &hot_ice::HotState, #(#args_no_receiver),*) #return_type {
                Self::#inner_fn_ident(state.ref_state(), #(#arg_names),*)
            }
            #input
        }
    } else {
        let original_inputs = &input.sig.inputs;
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(#original_inputs) #return_type {
                self.#inner_fn_ident(#(#arg_names),*)
            }
            #input
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn scale_factor(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());

    let vis = &input.vis;
    let return_type = &input.sig.output;

    let mut args_no_receiver = Vec::new();
    let mut arg_names = Vec::new();
    for arg in input.sig.inputs.iter().skip(1) {
        args_no_receiver.push(arg);
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                arg_names.push(&pat_ident.ident);
            }
        }
    }

    input.sig.ident = inner_fn_ident.clone();

    let expanded = if hot_state {
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(state: &hot_ice::HotState, #(#args_no_receiver),*) #return_type {
                Self::#inner_fn_ident(state.ref_state(), #(#arg_names),*)
            }
            #input
        }
    } else {
        let original_inputs = &input.sig.inputs;
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(#original_inputs) #return_type {
                self.#inner_fn_ident(#(#arg_names),*)
            }
            #input
        }
    };

    proc_macro::TokenStream::from(expanded)
}

enum FnType {
    Boot,
    Update,
    View,
    Subscription,
    Theme,
    Style,
    ScaleFactor,
    Unknown,
}

fn detect_fn_type(input: &syn::ItemFn) -> FnType {
    let return_type = &input.sig.output;
    let return_type_str = quote!(#return_type).to_string();
    let inputs = &input.sig.inputs;

    // Boot: 0 args, returns tuple
    if inputs.is_empty() {
        if let syn::ReturnType::Type(_, ty) = return_type {
            if let syn::Type::Tuple(_) = **ty {
                return FnType::Boot;
            }
        }
    }

    if inputs.len() == 1 {
        if return_type_str.contains("Element") {
            return FnType::View;
        }
        if return_type_str.contains("Subscription") {
            return FnType::Subscription;
        }
        if return_type_str.contains("Option") && return_type_str.contains("Theme") {
            return FnType::Theme;
        }
    }

    if inputs.len() == 2 {
        if return_type_str.contains("Task") {
            return FnType::Update;
        }
        if return_type_str.contains("Style") {
            return FnType::Style;
        }
        if return_type_str.contains("f32") {
            return FnType::ScaleFactor;
        }
    }

    FnType::Unknown
}

#[proc_macro_attribute]
pub fn hot_fn(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item_clone = item.clone();
    let input = parse_macro_input!(item_clone as syn::ItemFn);

    // Parse the attribute to determine if hot_state is enabled
    let attr_str = attr.to_string();
    let hot_state = attr_str.contains("hot_state");

    // For subscription, also check for not_hot/not-hot
    let is_hot = !attr_str.contains("not_hot") && !attr_str.contains("not-hot");

    let fn_type = detect_fn_type(&input);

    match fn_type {
        FnType::Boot => boot(hot_state, item),
        FnType::Update => update(hot_state, item),
        FnType::View => view(hot_state, item),
        FnType::Subscription => subscription(hot_state, is_hot, item),
        FnType::Theme => theme(hot_state, item),
        FnType::Style => style(hot_state, item),
        FnType::ScaleFactor => scale_factor(hot_state, item),
        FnType::Unknown => {
            let msg = "Unsupported function, supported functions are\n
                .boot\n
                .update\n
                .view\n
                .subscription\n
                .theme\n
                .style\n
                .scale_factor";

            let tokens = quote_spanned! {input.span() =>
                compile_error!(#msg);
            };
            tokens.into()
        }
    }
}
