# WGPUI HTTP client

The source candidate is version 0.1.0 and requires Rust 1.95 or newer. It accompanies WGPUI 0.4.0 and Kit 0.8.1; use the package from this workspace until it is published.

HTTP request interfaces, asynchronous bodies, proxy wrappers, test clients and release-download helpers used by WGPUI. This crate preserves the framework's executor-neutral HTTP interface while using released upstream Reqwest and async-tar packages.

The interface and helpers are adapted from gpui_http_client 0.2.2 under Apache-2.0. The multipart contract now uses stable reqwest types, available through this crate's reqwest reexport. Implement HttpClient in the application's networking integration; the framework does not select or initialize a network runtime.

Enable test-support for FakeHttpClient. Its handlers run on the caller's executor and can return streaming AsyncBody values.
