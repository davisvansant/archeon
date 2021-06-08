use hyper::body::{to_bytes, Bytes};
use hyper::client::connect::HttpConnector;
use hyper::header::{HeaderValue, CONTENT_LENGTH};
use hyper::{Body, Client, Request, Uri};
use hyper_tls::HttpsConnector;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use std::path::{Path, PathBuf};
use std::str::FromStr;

pub(crate) struct Transfer {
    client: Client<HttpsConnector<HttpConnector>, Body>,
    uri: Uri,
    filename: String,
    pub(crate) initialized: bool,
}

impl Transfer {
    pub(crate) async fn init(uri: &str) -> Transfer {
        let https = HttpsConnector::new();
        let client = Client::builder().build(https);
        let uri = Uri::from_str(uri).expect("Unable to parse URI!");
        let filename = Self::get_filename(&uri).await;

        Transfer {
            client,
            uri,
            filename,
            initialized: true,
        }
    }

    pub(crate) async fn launch(&self) {
        let uri = self.uri.to_owned();
        self.client.get(uri).await;
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

    async fn body_to_bytes(body: Body) -> Result<Bytes, hyper::Error> {
        let bytes = to_bytes(body).await?;
        Ok(bytes)
    }

    async fn create_file(
        file: &str,
        bytes: Bytes,
        content_length: HeaderValue,
    ) -> Result<(), std::io::Error> {
        let mut file_path = PathBuf::with_capacity(15);
        file_path.push(file);

        let mut file = File::create(&file_path).await?;
        file.write_all(&bytes).await?;

        let mut initial_size = Transfer::get_file_length(&file_path).await?;
        let content_length_str = content_length.to_str().unwrap();
        let total_size = u64::from_str(content_length_str).unwrap();
        let progress_bar = ProgressBar::new(total_size);

        while initial_size < total_size {
            let current_size = Transfer::get_file_length(&file_path).await?;
            initial_size = current_size;
            progress_bar.set_position(current_size);
        }

        progress_bar.finish();

        Ok(())
    }

    async fn get_file_length(file: &Path) -> Result<u64, std::io::Error> {
        let open_file = File::open(file).await?;
        let open_file_metadata = open_file.metadata().await?;
        Ok(open_file_metadata.len())
    }

    async fn get_filename(uri: &Uri) -> String {
        match uri.path_and_query() {
            None => panic!("cannot get filename from URI!"),
            Some(path_and_query) => {
                let filename = path_and_query.as_str().rsplit_once("/");
                match filename {
                    None => path_and_query.as_str().to_string(),
                    Some(filename) => filename.1.to_string(),
                }
            }
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
    async fn launch() -> Result<(), hyper::Error> {
        let test_mock_url = mockito::server_url();
        let test_transfer = Transfer::init(&test_mock_url).await;
        let mock = mock("GET", "/")
            .with_status(200)
            .with_header("Content-Length", "100000")
            .with_body("")
            .create();
        test_transfer.launch().await;
        mock.assert();
        assert!(mock.matched());
        Ok(())
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

    #[tokio::test(flavor = "multi_thread")]
    async fn body_to_bytes() -> Result<(), hyper::Error> {
        let test_body = Body::from("test_body");
        let test_body_to_bytes = Transfer::body_to_bytes(test_body).await?;
        assert_eq!(test_body_to_bytes.len(), 9);
        assert_eq!(test_body_to_bytes, Bytes::from("test_body"));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_file() -> Result<(), std::io::Error> {
        let test_bytes = Bytes::from("test_bytes");
        let test_content_length = HeaderValue::from_static("10");
        let test_filename = "test_create_file.txt";
        if let Ok(()) = Transfer::create_file(test_filename, test_bytes, test_content_length).await
        {
            let test_file = File::open(test_filename).await?;
            let test_file_metadata = test_file.metadata().await?;
            assert_eq!(test_file_metadata.is_file(), true);
            assert_eq!(test_file_metadata.len(), 10);
            tokio::fs::remove_file(&test_filename).await?;
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_file_length() -> Result<(), std::io::Error> {
        let test_bytes = Bytes::from("test_bytes");
        let test_content_length = HeaderValue::from_static("10");
        let test_filename = "test_get_file_length.txt";
        if let Ok(()) = Transfer::create_file(test_filename, test_bytes, test_content_length).await
        {
            let test_file_path = PathBuf::from(test_filename);
            let test_file = Transfer::get_file_length(&test_file_path).await?;
            assert_eq!(test_file, 10);
            tokio::fs::remove_file(&test_file_path).await?;
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_filename() {
        let test_uri = "http://some_test_authority/with/path/and/query.extension";
        let test_transfer = Transfer::init(test_uri).await;
        assert_eq!(test_transfer.filename, "query.extension");
    }
}
