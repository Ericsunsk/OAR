macro_rules! apply_headers {
    ($builder:expr, $headers:expr) => {{
        let mut builder = $builder;
        for (name, value) in $headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        builder
    }};
}

mod async_client;
mod blocking;
mod types;

pub use async_client::ReqwestAsyncHttpClient;
pub use blocking::ReqwestBlockingHttpClient;
pub use types::{AsyncHttpClient, HttpClient, HttpClientFailure, HttpRequest, HttpResponse};
