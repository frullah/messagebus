#![recursion_limit = "128"]

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use std::fmt::Write;
use syn::parse::{Parse, ParseStream};
use syn::{parenthesized, Result};
use syn::{punctuated::Punctuated, token::Comma, DeriveInput};

fn shared_part(_ast: &syn::DeriveInput, has_shared: bool) -> proc_macro2::TokenStream {
    if has_shared {
        quote! {
            fn as_shared_ref(&self) -> std::option::Option<&dyn messagebus::SharedMessage> {Some(self)}
            fn as_shared_mut(&mut self) -> std::option::Option<&mut dyn messagebus::SharedMessage>{Some(self)}
            fn as_shared_boxed(self: Box<Self>) -> Result<Box<dyn messagebus::SharedMessage>, Box<dyn messagebus::Message>> {Ok(self)}
            fn as_shared_arc(self: std::sync::Arc<Self>) -> Option<std::sync::Arc<dyn messagebus::SharedMessage>>{Some(self)}
        }
    } else {
        quote! {
            fn as_shared_ref(&self) -> std::option::Option<&dyn messagebus::SharedMessage> {None}
            fn as_shared_mut(&mut self) -> std::option::Option<&mut dyn messagebus::SharedMessage> {None}
            fn as_shared_boxed(self: Box<Self>) -> Result<Box<dyn messagebus::SharedMessage>, Box<dyn messagebus::Message>> {Err(self)}
            fn as_shared_arc(self: std::sync::Arc<Self>) -> Option<std::sync::Arc<dyn messagebus::SharedMessage>> {None}
        }
    }
}

fn clone_part(ast: &syn::DeriveInput, has_clone: bool) -> proc_macro2::TokenStream {
    let name = &ast.ident;
    let (_, ty_generics, _) = ast.generics.split_for_impl();

    if has_clone {
        quote! {
            fn try_clone_into(&self, into: &mut dyn core::any::Any) -> bool {
                let into = if let Some(inner) = into.downcast_mut::<Option<#name #ty_generics>>() {
                    inner
                } else {
                    return false;
                };

                into.replace(core::clone::Clone::clone(self));
                true
            }
            fn try_clone_boxed(&self) -> std::option::Option<std::boxed::Box<dyn messagebus::Message>>{
                Some(Box::new(core::clone::Clone::clone(self)))
            }
        }
    } else {
        quote! {
            fn try_clone_into(&self, into: &mut dyn core::any::Any) -> bool {false}
            fn try_clone_boxed(&self) -> std::option::Option<std::boxed::Box<dyn messagebus::Message>>{ None }
        }
    }
}

fn type_tag_part(
    ast: &syn::DeriveInput,
    type_tag: Option<String>,
    namespace: Option<String>,
) -> proc_macro2::TokenStream {
    let class_name = &ast.ident;
    let name = if let Some(tt) = type_tag {
        tt
    } else if let Some(ns) = namespace {
        format!("{}::{}", ns, class_name)
    } else {
        class_name.to_string()
    };

    let (_, ty_generics, where_clause) = ast.generics.split_for_impl();
    let mut impl_generics = ast.generics.clone();

    let mut type_name = String::new();
    let mut type_values = String::from("(");
    let mut need_close = false;

    write!(&mut type_name, "{}", name).unwrap();
    for mut param in impl_generics.params.pairs_mut() {
        match &mut param.value_mut() {
            syn::GenericParam::Lifetime(_) => continue,
            syn::GenericParam::Type(param) => {
                if !need_close {
                    type_name.push('<');
                    need_close = true;
                } else {
                    type_name.push(',');
                    type_values.push(',');
                }

                type_name.push_str("{}");

                write!(
                    &mut type_values,
                    "<{} as messagebus::TypeTagged>::type_tag_()",
                    param.ident
                )
                .unwrap();

                let bound: syn::TypeParamBound = syn::parse_str("messagebus::TypeTagged").unwrap();
                param.bounds.push(bound);
            }
            syn::GenericParam::Const(_param) => {
                unimplemented!()
            }
        }
    }

    if need_close {
        type_name.push('>');
    }

    if type_values.len() > 1 {
        type_values.push_str(",)");

        let type_values: syn::ExprTuple = syn::parse_str(&type_values).unwrap();
        let type_values = type_values.elems;

        quote! {
            impl #impl_generics messagebus::TypeTagged for #class_name #ty_generics #where_clause {
                fn type_tag_() -> messagebus::TypeTag { format!(#type_name, #type_values).into() }
                fn type_tag(&self) -> messagebus::TypeTag {  Self::type_tag_() }
                fn type_name(&self) -> std::borrow::Cow<str> {  Self::type_tag_() }
            }
        }
    } else {
        quote! {
            impl #impl_generics messagebus::TypeTagged for #class_name #ty_generics #where_clause {
                fn type_tag_() -> messagebus::TypeTag { #type_name.into() }
                fn type_tag(&self) -> messagebus::TypeTag {  Self::type_tag_() }
                fn type_name(&self) -> std::borrow::Cow<str> {  Self::type_tag_() }
            }
        }
    }
}

struct TypeTag {
    pub inner: syn::LitStr,
}

impl Parse for TypeTag {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        let punctuated = Punctuated::<syn::LitStr, Comma>::parse_terminated(&content)?;

        let inner = punctuated.pairs().map(|x| x.into_value()).next();

        Ok(TypeTag {
            inner: inner.unwrap().to_owned(),
        })
    }
}

#[derive(Default, Debug)]
struct Tags {
    has_clone: bool,
    has_shared: bool,
}

impl Tags {
    pub fn add(&mut self, other: Tags) {
        self.has_clone = self.has_clone || other.has_clone;
        self.has_shared = self.has_shared || other.has_shared;
    }
}

impl Parse for Tags {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut has_shared = false;
        let mut has_clone = false;

        let content;
        parenthesized!(content in input);
        let punctuated = Punctuated::<syn::Ident, Comma>::parse_terminated(&content)?;

        for pair in punctuated.pairs() {
            let val = pair.into_value().to_string();
            match val.as_str() {
                "shared" => has_shared = true,
                "clone" => has_clone = true,
                _ => (),
            }
        }

        Ok(Tags {
            has_clone,
            has_shared,
        })
    }
}

#[proc_macro_derive(Message, attributes(type_tag, message, namespace))]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let mut tags = Tags::default();
    let mut type_tag = None;
    let mut namespace = None;

    let ast: DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;
    let (_, ty_generics, where_clause) = ast.generics.split_for_impl();
    for attr in &ast.attrs {
        if let Some(i) = attr.path.get_ident() {
            match i.to_string().as_str() {
                "message" => {
                    let tt: Tags = syn::parse2(attr.tokens.clone()).unwrap();
                    tags.add(tt);
                }

                "type_tag" => {
                    let tt: TypeTag = syn::parse2(attr.tokens.clone()).unwrap();
                    type_tag = Some(tt.inner.value());
                }

                "namespace" => {
                    let tt: TypeTag = syn::parse2(attr.tokens.clone()).unwrap();
                    namespace = Some(tt.inner.value());
                }

                _ => (),
            }
        }
    }

    let mut impl_generics = ast.generics.clone();
    for mut param in impl_generics.params.pairs_mut() {
        match &mut param.value_mut() {
            syn::GenericParam::Lifetime(_) => {}
            syn::GenericParam::Type(param) => {
                let bound: syn::TypeParamBound =
                    syn::parse_str("messagebus::MessageBounds").unwrap();
                param.bounds.push(bound);
            }
            syn::GenericParam::Const(_param) => {}
        }
    }

    let type_tag_part = type_tag_part(&ast, type_tag, namespace);
    let shared_part = shared_part(&ast, tags.has_shared);
    let clone_part = clone_part(&ast, tags.has_clone);

    let tokens = quote! {
        #type_tag_part

        impl #impl_generics messagebus::Message for #name #ty_generics #where_clause {
            fn as_any_ref(&self) -> &dyn core::any::Any {self}
            fn as_any_mut(&mut self) -> &mut dyn core::any::Any {self}
            fn as_any_boxed(self: std::boxed::Box<Self>) -> std::boxed::Box<dyn core::any::Any> {self}
            fn as_any_arc(self: std::sync::Arc<Self>) -> std::sync::Arc<dyn core::any::Any> {self}

            #shared_part
            #clone_part
        }
    };

    tokens.into()
}

#[proc_macro_derive(Error, attributes(type_tag, namespace))]
pub fn derive_error(input: TokenStream) -> TokenStream {
    let mut type_tag = None;
    let mut namespace = None;

    let ast: DeriveInput = syn::parse(input).unwrap();
    for attr in &ast.attrs {
        if let Some(i) = attr.path.get_ident() {
            match i.to_string().as_str() {
                "type_tag" => {
                    let tt: TypeTag = syn::parse2(attr.tokens.clone()).unwrap();
                    type_tag = Some(tt.inner.value());
                }

                "namespace" => {
                    let tt: TypeTag = syn::parse2(attr.tokens.clone()).unwrap();
                    namespace = Some(tt.inner.value());
                }

                _ => (),
            }
        }
    }

    let type_tag_part = type_tag_part(&ast, type_tag, namespace);
    let tokens = quote! {
        #type_tag_part
    };

    tokens.into()
}
