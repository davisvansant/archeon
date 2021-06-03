use hyper::client::connect::HttpConnector;
use hyper::{Body, Client, Uri};
use hyper_tls::HttpsConnector;

use std::str::FromStr;

pub(crate) struct Transfer {
    client: Client<HttpsConnector<HttpConnector>, Body>,
    uri: Uri,
    pub(crate) initialized: bool,
}

impl Transfer {
    pub(crate) async fn init(uri: &str) -> Transfer {
        let https = HttpsConnector::new();
        let client = Client::builder().build(https);
        let uri = Uri::from_str(uri).expect("Unable to parse URI!");

        Transfer {
            client,
            uri,
            initialized: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn transfer() {
        let test_uri = "http://some_test_authority/with/path/and/query";
        let test_transfer = Transfer::init(test_uri).await;
        let test_uri_parts = test_transfer.uri.into_parts();
        assert_eq!(test_uri_parts.scheme.unwrap().as_str(), "http");
        assert_eq!(
            test_uri_parts.authority.unwrap().as_str(),
            "some_test_authority",
        );
        assert_eq!(
            test_uri_parts.path_and_query.unwrap().as_str(),
            "/with/path/and/query",
        );
        assert_eq!(test_transfer.initialized, true);
    }
}
