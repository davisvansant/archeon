use hyper::client::connect::HttpConnector;
use hyper::header::{HeaderValue, CONTENT_LENGTH};
use hyper::{Body, Client, Request, Uri};
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

    async fn get_content_length(&self) -> HeaderValue {
        let request = Request::head(&self.uri)
            .body(Body::empty())
            .expect("Could not Build Request!");

        let response = self.client.request(request).await;
        let response_parts = match response {
            Ok(response) => response.into_parts(),
            Err(error) => panic!("{}", error),
        };
        let content_length = response_parts.0.headers.get(CONTENT_LENGTH);
        if let Some(header_value) = content_length {
            header_value.to_owned()
        } else {
            panic!("Could not retrieve 'Content-Length' header!")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;

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

    #[tokio::test(flavor = "multi_thread")]
    async fn get_content_length() -> Result<(), hyper::Error> {
        let test_mock_url = mockito::server_url();
        let test_transfer = Transfer::init(&test_mock_url).await;
        let mock = mock("HEAD", "/")
            .with_status(200)
            .with_header("Content-Length", "100000")
            .with_body("")
            .create();
        let test_content_length_value = test_transfer.get_content_length().await;
        mock.assert();
        assert!(mock.matched());
        assert_eq!(test_content_length_value.to_str().unwrap(), "100000");
        Ok(())
    }
}
