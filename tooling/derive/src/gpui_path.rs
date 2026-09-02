use proc_macro2::TokenStream;
use quote::quote;

/// Crate path used in generated impls (`Action`, `IntoElement`, …).
///
/// Always `wgpui`. Dependents import the published package as `wgpui`, not
/// `gpui`; using `CARGO_PKG_NAME != "wgpui"` as a signal to emit `gpui::`
/// broke Syntax/Mosaic `actions!` expansions.
pub fn gpui_crate() -> TokenStream {
    quote!(wgpui)
}
