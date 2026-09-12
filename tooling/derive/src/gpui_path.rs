use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;

/// Resolve the actual dependency key, including applications using only Kit.
pub fn gpui_crate() -> TokenStream {
    for package in ["wgpui", "wgpui-kit"] {
        match crate_name(package) {
            Ok(FoundCrate::Itself) => {
                let name = Ident::new(&package.replace('-', "_"), Span::call_site());
                return quote!(#name);
            }
            Ok(FoundCrate::Name(name)) => {
                let name = Ident::new(&name, Span::call_site());
                return quote!(::#name);
            }
            Err(_) => continue,
        }
    }
    quote!(::wgpui)
}
