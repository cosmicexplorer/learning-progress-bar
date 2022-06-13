/* NB: Any nightly-only features go here >=]! */
#![deny(warnings)]
// Enable all clippy lints except for many of the pedantic ones. It's a shame this needs to be
// copied and pasted across crates, but there doesn't appear to be a way to include inner attributes
// from a common source.
#![deny(
  clippy::all,
  clippy::default_trait_access,
  clippy::expl_impl_clone_on_copy,
  clippy::if_not_else,
  clippy::needless_continue,
  clippy::single_match_else,
  clippy::unseparated_literal_suffix,
  clippy::used_underscore_binding
)]
// We only use unsafe pointer dereference in our no_mangle exposed API, but it is nicer to list
// just the one minor call as unsafe, than to mark the whole function as unsafe which may hide
// other unsafeness.
#![allow(clippy::not_unsafe_ptr_arg_deref)]
/* FIXME: remove these!!! */
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use proc_macro;
use proc_macro2::{Delimiter, Group, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::{quote, quote_spanned, ToTokens, TokenStreamExt};
use syn::{
  self, braced,
  parse::{Parse, ParseStream},
  parse_macro_input,
  punctuated::Punctuated,
  spanned::Spanned,
  token, Expr, Field, Ident, Token, Type, Visibility,
};

use std::iter::Extend;

#[derive(Clone)]
struct NamedFieldBinding {
  pub ident: Ident,
  pub colon: token::Colon,
  pub expr: Expr,
}

impl Parse for NamedFieldBinding {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    Ok(NamedFieldBinding {
      ident: input.parse()?,
      colon: input.parse()?,
      expr: input.parse()?,
    })
  }
}

/* (from https://docs.rs/syn/1.0.13/syn/parse/index.html) */
struct BindInput {
  pub ident: Ident,
  pub brace_token: token::Brace,
  pub fields: Punctuated<NamedFieldBinding, Token![,]>,
  pub swooshy_le_token: token::Le,
  pub source_expr: Expr,
}

impl Parse for BindInput {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let content;
    Ok(BindInput {
      ident: input.parse()?,
      brace_token: braced!(content in input),
      fields: content.parse_terminated(NamedFieldBinding::parse)?,
      swooshy_le_token: input.parse()?,
      source_expr: input.parse()?,
    })
  }
}

#[proc_macro]
pub fn bind(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
  /*
   *   <xxx>![let ThriftChunk { capacity: (capacity * 2), .. } = chunk]
   * into:
   *   let ThriftChunk { capacity, .. } = chunk; let capacity = (capacity * 2);
   */
  let BindInput {
    ident,
    fields,
    source_expr,
    ..
  } = parse_macro_input!(input as BindInput);

  let captured_fields = {
    let mut tok_str = TokenStream::new();
    for NamedFieldBinding { ident, .. } in fields.iter().cloned() {
      tok_str.append::<Ident>(ident.into());
      tok_str.append(Punct::new(',', Spacing::Alone));
    }
    tok_str
  };

  let bind_statements = {
    let mut tok_str = TokenStream::new();
    /* tok_str.append_separated(fields.into_iter(), ) */
    for NamedFieldBinding { ident, expr, .. } in fields.into_iter() {
      tok_str.append(Ident::new("let", Span::call_site()));
      ident.to_tokens(&mut tok_str);
      tok_str.append(Punct::new('=', Spacing::Alone));
      expr.to_tokens(&mut tok_str);
      tok_str.append(Punct::new(';', Spacing::Alone));
    }
    tok_str
  };

  let expanded: TokenStream = quote! {
    let #ident {
      #(
        #captured_fields
      ),+
      ,
      ..
    } = #source_expr;
    #bind_statements
  };
  proc_macro::TokenStream::from(expanded)
}


#[cfg(test)]
mod tests {
  use super::*;

  struct S {
    pub a: usize,
    pub b: String,
  }

  #[test]
  fn bind_single_field() {
    let s = S { a: 3, b: "".to_string() };

    bind!(let S { a, .. } = s);

    assert_eq!(2 + 2, 4);
  }
}
