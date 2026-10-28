mod derive_action;
mod derive_app_context;
mod derive_into_element;
mod derive_render;
mod derive_visual_context;
mod property_test;
mod register_action;
mod styles;
mod test;
mod wgpui_path;

#[cfg(any(feature = "inspector", debug_assertions))]
mod derive_inspector_reflection;

use proc_macro::TokenStream;
use syn::{DeriveInput, Ident};

/// Runs an explicitly unsafe constructor using WGPUI's dependency path.
#[proc_macro_attribute]
pub fn ctor(args: TokenStream, input: TokenStream) -> TokenStream {
    let wgpui = wgpui_path::wgpui_crate();
    let args = proc_macro2::TokenStream::from(args);
    let input = proc_macro2::TokenStream::from(input);
    let separator = (!args.is_empty()).then(|| quote::quote!(,));

    quote::quote! {
        #[#wgpui::private::ctor::ctor(crate_path = #wgpui::private::ctor #separator #args)]
        #input
    }
    .into()
}

/// `Action` derive macro - see the trait documentation for details.
#[proc_macro_derive(Action, attributes(action))]
pub fn derive_action(input: TokenStream) -> TokenStream {
    derive_action::derive_action(input)
}

/// This can be used to register an action with the WGPUI runtime when you want to manually implement
/// the `Action` trait. Typically you should use the `Action` derive macro or `actions!` macro
/// instead.
#[proc_macro]
pub fn register_action(ident: TokenStream) -> TokenStream {
    register_action::register_action(ident)
}

/// #[derive(IntoElement)] is used to create a Component out of anything that implements
/// the `RenderOnce` trait.
#[proc_macro_derive(IntoElement)]
pub fn derive_into_element(input: TokenStream) -> TokenStream {
    derive_into_element::derive_into_element(input)
}

#[proc_macro_derive(Render)]
#[doc(hidden)]
pub fn derive_render(input: TokenStream) -> TokenStream {
    derive_render::derive_render(input)
}

/// #[derive(AppContext)] is used to create a context out of anything that holds a `&mut App`
/// Note that a `#[app]` attribute is required to identify the variable holding the &mut App.
///
/// Failure to add the attribute causes a compile error:
///
/// ```compile_fail
/// # #[macro_use] extern crate wgpui_derive;
/// # #[macro_use] extern crate wgpui;
/// #[derive(AppContext)]
/// struct MyContext<'a> {
///     app: &'a mut wgpui::App
/// }
/// ```
#[proc_macro_derive(AppContext, attributes(app))]
pub fn derive_app_context(input: TokenStream) -> TokenStream {
    derive_app_context::derive_app_context(input)
}

/// #[derive(VisualContext)] is used to create a visual context out of anything that holds a `&mut Window` and
/// implements `AppContext`
/// Note that a `#[app]` and a `#[window]` attribute are required to identify the variables holding the &mut App,
/// and &mut Window respectively.
///
/// Failure to add both attributes causes a compile error:
///
/// ```compile_fail
/// # #[macro_use] extern crate wgpui_derive;
/// # #[macro_use] extern crate wgpui;
/// #[derive(VisualContext)]
/// struct MyContext<'a, 'b> {
///     #[app]
///     app: &'a mut wgpui::App,
///     window: &'b mut wgpui::Window
/// }
/// ```
///
/// ```compile_fail
/// # #[macro_use] extern crate wgpui_derive;
/// # #[macro_use] extern crate wgpui;
/// #[derive(VisualContext)]
/// struct MyContext<'a, 'b> {
///     app: &'a mut wgpui::App,
///     #[window]
///     window: &'b mut wgpui::Window
/// }
/// ```
#[proc_macro_derive(VisualContext, attributes(window, app))]
pub fn derive_visual_context(input: TokenStream) -> TokenStream {
    derive_visual_context::derive_visual_context(input)
}

/// Used by WGPUI to generate the style helpers.
#[proc_macro]
#[doc(hidden)]
pub fn style_helpers(input: TokenStream) -> TokenStream {
    styles::style_helpers(input)
}

/// Generates methods for visibility styles.
#[proc_macro]
pub fn visibility_style_methods(input: TokenStream) -> TokenStream {
    styles::visibility_style_methods(input)
}

/// Generates methods for margin styles.
#[proc_macro]
pub fn margin_style_methods(input: TokenStream) -> TokenStream {
    styles::margin_style_methods(input)
}

/// Generates methods for padding styles.
#[proc_macro]
pub fn padding_style_methods(input: TokenStream) -> TokenStream {
    styles::padding_style_methods(input)
}

/// Generates methods for position styles.
#[proc_macro]
pub fn position_style_methods(input: TokenStream) -> TokenStream {
    styles::position_style_methods(input)
}

/// Generates methods for overflow styles.
#[proc_macro]
pub fn overflow_style_methods(input: TokenStream) -> TokenStream {
    styles::overflow_style_methods(input)
}

/// Generates methods for cursor styles.
#[proc_macro]
pub fn cursor_style_methods(input: TokenStream) -> TokenStream {
    styles::cursor_style_methods(input)
}

/// Generates methods for border styles.
#[proc_macro]
pub fn border_style_methods(input: TokenStream) -> TokenStream {
    styles::border_style_methods(input)
}

/// Generates methods for box shadow styles.
#[proc_macro]
pub fn box_shadow_style_methods(input: TokenStream) -> TokenStream {
    styles::box_shadow_style_methods(input)
}

/// `#[wgpui::test]` can be used to annotate test functions that run with WGPUI support.
///
/// It supports both synchronous and asynchronous tests, and can provide you with
/// as many `TestAppContext` instances as you need.
/// The output contains a `#[test]` annotation so this can be used with any existing
/// test harness (`cargo test` or `cargo-nextest`).
///
/// ```
/// #[wgpui::test]
/// async fn test_foo(mut cx: &TestAppContext) { }
/// ```
///
/// In addition to passing a TestAppContext, you can also ask for a `StdRnd` instance.
/// this will be seeded with the `SEED` environment variable and is used internally by
/// the ForegroundExecutor and BackgroundExecutor to run tasks deterministically in tests.
/// Using the same `StdRng` for behavior in your test will allow you to exercise a wide
/// variety of scenarios and interleavings just by changing the seed.
///
/// # Arguments
///
/// - `#[wgpui::test]` with no arguments runs once with the seed `0` or `SEED` env var if set.
/// - `#[wgpui::test(seed = 10)]` runs once with the seed `10`.
/// - `#[wgpui::test(seeds(10, 20, 30))]` runs three times with seeds `10`, `20`, and `30`.
/// - `#[wgpui::test(iterations = 5)]` runs five times, providing as seed the values in the range `0..5`.
/// - `#[wgpui::test(retries = 3)]` runs up to four times if it fails to try and make it pass.
/// - `#[wgpui::test(on_failure = "crate::test::report_failure")]` will call the specified function after the
///   tests fail so that you can write out more detail about the failure.
///
/// You can combine `iterations = ...` with `seeds(...)`:
/// - `#[wgpui::test(iterations = 5, seed = 10)]` is equivalent to `#[wgpui::test(seeds(0, 1, 2, 3, 4, 10))]`.
/// - `#[wgpui::test(iterations = 5, seeds(10, 20, 30)]` is equivalent to `#[wgpui::test(seeds(0, 1, 2, 3, 4, 10, 20, 30))]`.
/// - `#[wgpui::test(seeds(10, 20, 30), iterations = 5)]` is equivalent to `#[wgpui::test(seeds(0, 1, 2, 3, 4, 10, 20, 30))]`.
///
/// # Environment Variables
///
/// - `SEED`: sets a seed for the first run
/// - `ITERATIONS`: forces the value of the `iterations` argument
#[proc_macro_attribute]
pub fn test(args: TokenStream, function: TokenStream) -> TokenStream {
    test::test(args, function)
}

/// Legacy property-testing attribute; unsupported by WGPUI 0.4.0.
///
/// The retained implementation targets an older Proptest scheduler bridge
/// (`proptest`, `seed_strategy`, and `run_test_once`) that this framework does
/// not expose. It is not reexported as `wgpui::property_test`; invoking the
/// derive attribute directly does not restore that missing bridge.
///
/// ```compile_fail
/// #[wgpui::property_test]
/// fn arithmetic(x: i32, y: i32) {
///     assert!(x == y || x < y || x > y);
/// }
/// ```
///
/// Use Proptest directly for pure data properties with an explicit Proptest
/// dev-dependency. Use `#[wgpui::test]` for WGPUI's deterministic UI scheduler
/// and enable the framework's `test-support` feature for its public helpers.
/// See the [Proptest book](https://proptest-rs.github.io/proptest/intro.html).
#[proc_macro_attribute]
pub fn property_test(args: TokenStream, function: TokenStream) -> TokenStream {
    property_test::test(args.into(), function.into()).into()
}

/// When added to a trait, `#[derive_inspector_reflection]` generates a module which provides
/// enumeration and lookup by name of all methods that have the shape `fn method(self) -> Self`.
/// This is used by the inspector so that it can use the builder methods in `Styled` and
/// `StyledExt`.
///
/// The generated module will have the name `<snake_case_trait_name>_reflection` and contain the
/// following functions:
///
/// ```ignore
/// pub fn methods::<T: TheTrait + 'static>() -> Vec<wgpui::inspector_reflection::FunctionReflection<T>>;
///
/// pub fn find_method::<T: TheTrait + 'static>() -> Option<wgpui::inspector_reflection::FunctionReflection<T>>;
/// ```
///
/// The `invoke` method on `FunctionReflection` will run the method. `FunctionReflection` also
/// provides the method's documentation.
#[cfg(any(feature = "inspector", debug_assertions))]
#[proc_macro_attribute]
pub fn derive_inspector_reflection(_args: TokenStream, input: TokenStream) -> TokenStream {
    derive_inspector_reflection::derive_inspector_reflection(_args, input)
}

pub(crate) fn get_simple_attribute_field(ast: &DeriveInput, name: &'static str) -> Option<Ident> {
    match &ast.data {
        syn::Data::Struct(data_struct) => data_struct
            .fields
            .iter()
            .find(|field| field.attrs.iter().any(|attr| attr.path().is_ident(name)))
            .map(|field| field.ident.clone().unwrap()),
        syn::Data::Enum(_) => None,
        syn::Data::Union(_) => None,
    }
}

/// Measures a synchronous UI benchmark through WGPUI's benchmark context.
#[proc_macro_attribute]
pub fn bench(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(proc_macro2::Span::call_site(), "bench takes no arguments")
            .to_compile_error()
            .into();
    }
    let function = syn::parse_macro_input!(input as syn::ItemFn);
    if function.sig.asyncness.is_some() || function.sig.inputs.len() != 1 {
        return syn::Error::new_spanned(
            &function.sig,
            "bench requires one synchronous &mut BenchAppContext argument",
        )
        .to_compile_error()
        .into();
    }
    let wgpui = wgpui_path::wgpui_crate();
    let name = &function.sig.ident;
    let attributes = &function.attrs;
    let visibility = &function.vis;
    let body = &function.block;
    let parameter = &function.sig.inputs[0];
    quote::quote! {
        #(#attributes)*
        #visibility fn #name(criterion: &mut #wgpui::private::criterion::Criterion) {
            fn setup(#parameter) #body
            criterion.bench_function(stringify!(#name), |bencher| {
                let mut context = #wgpui::BenchAppContext::new(bencher);
                setup(&mut context);
                context.teardown();
            });
        }
    }
    .into()
}
