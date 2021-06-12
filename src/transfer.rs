use hyper::body::{to_bytes, Bytes};
use hyper::client::connect::HttpConnector;
use hyper::header::{HeaderValue, CONTENT_LENGTH};
use hyper::{Body, Client, Request, Uri};
use hyper_tls::HttpsConnector;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use tokio::fs::{create_dir_all, File};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use std::env::temp_dir;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub(crate) struct Transfer {
    client: Client<HttpsConnector<HttpConnector>, Body>,
    uri: Uri,
    filename: PathBuf,
    temp_dir: PathBuf,
    pub(crate) initialized: bool,
}

impl Transfer {
    pub(crate) async fn init(uri: &str) -> Transfer {
        let https = HttpsConnector::new();
        let client = Client::builder().build(https);
        let uri = Uri::from_str(uri).expect("Unable to parse URI!");
        let filename = Self::get_filename(&uri).await;
        let temp_dir = Self::create_temp_dir().await;

        Transfer {
            client,
            uri,
            filename,
            temp_dir,
            initialized: true,
        }
    }

    pub(crate) async fn launch(&self) {
        let uri = self.uri.to_owned();
        let content_length = self.get_content_length().await;
        match self.client.get(uri).await {
            Ok(response) => {
                let response_body = response.into_body();
                let bytes = Self::body_to_bytes(response_body).await.unwrap();
                // self.create_file(bytes, content_length).await.unwrap();
            }
            Err(error) => panic!("we need to retry here {}", error),
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

    async fn body_to_bytes(body: Body) -> Result<Bytes, hyper::Error> {
        let bytes = to_bytes(body).await?;
        Ok(bytes)
    }

    async fn create_file(
        &self,
        bytes: Bytes,
        content_length: HeaderValue,
    ) -> Result<(), std::io::Error> {
        let mut temp_dir_path = PathBuf::with_capacity(15);
        temp_dir_path.push(&self.temp_dir);
        temp_dir_path.push(&self.filename);

        let mut file = File::create(&temp_dir_path).await?;
        file.write_all(&bytes).await?;

        let mut initial_size = self.get_file_length().await?;
        let content_length_str = content_length.to_str().unwrap();
        let total_size = u64::from_str(content_length_str).unwrap();
        let progress_bar = ProgressBar::new(total_size);

        while initial_size < total_size {
            let current_size = self.get_file_length().await?;
            initial_size = current_size;
            progress_bar.set_position(current_size);
        }

        progress_bar.finish();

        Ok(())
    }

    async fn get_file_length(&self) -> Result<u64, std::io::Error> {
        let mut temp_dir_path = PathBuf::with_capacity(15);
        temp_dir_path.push(&self.temp_dir);
        temp_dir_path.push(&self.filename);

        let open_file = File::open(&temp_dir_path).await?;
        let open_file_metadata = open_file.metadata().await?;
        Ok(open_file_metadata.len())
    }

    async fn get_filename(uri: &Uri) -> PathBuf {
        match uri.path_and_query() {
            None => panic!("cannot get filename from URI!"),
            Some(path_and_query) => {
                let filename = path_and_query.as_str().rsplit_once("/");
                match filename {
                    None => Self::create_path(path_and_query.as_str()).await,
                    Some(filename) => Self::create_path(filename.1).await,
                }
            }
        }
    }

    async fn create_path(filename: &str) -> PathBuf {
        let mut path = PathBuf::with_capacity(15);
        path.push(filename);
        path
    }

    async fn create_temp_dir() -> PathBuf {
        let mut path = PathBuf::with_capacity(15);
        let temp_dir = temp_dir();
        let temp_dir_path = "archeon";

        path.push(temp_dir);
        path.push(temp_dir_path);

        match create_dir_all(&path).await {
            Ok(()) => path,
            Err(error) => panic!("{}", error),
        }
    }

    async fn install_package(&self) -> Result<(), std::io::Error> {
        let command = Command::new("dpkg")
            .arg("--install")
            .arg(&self.filename)
            .current_dir(&self.temp_dir)
            .output()
            .await?;

        println!("{:?}", command.status);
        println!("{:#?}", String::from_utf8(command.stdout));
        println!("{:#?}", String::from_utf8(command.stderr));

        Ok(())
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
            .with_header("content-length", "9")
            .with_body(b"test_body")
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
        let test_uri = "http://test-create-file/test_create_file.txt";
        let test_transfer = Transfer::init(test_uri).await;
        if let Ok(()) = Transfer::create_file(&test_transfer, test_bytes, test_content_length).await
        {
            let mut temp_dir_path = PathBuf::with_capacity(15);
            temp_dir_path.push(&test_transfer.temp_dir);
            temp_dir_path.push(&test_transfer.filename);
            let test_file = File::open(&temp_dir_path).await?;
            let test_file_metadata = test_file.metadata().await?;
            assert_eq!(test_file_metadata.is_file(), true);
            assert_eq!(test_file_metadata.len(), 10);
            tokio::fs::remove_file(&temp_dir_path).await?;
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_file_length() -> Result<(), std::io::Error> {
        let test_bytes = Bytes::from("test_bytes");
        let test_content_length = HeaderValue::from_static("10");
        let test_uri = "http://get-file-length/test_get_file_length.txt";
        let test_transfer = Transfer::init(test_uri).await;
        if let Ok(()) = Transfer::create_file(&test_transfer, test_bytes, test_content_length).await
        {
            let test_file = test_transfer.get_file_length().await?;
            assert_eq!(test_file, 10);
            let mut temp_dir_path = PathBuf::with_capacity(15);
            temp_dir_path.push(&test_transfer.temp_dir);
            temp_dir_path.push(&test_transfer.filename);
            tokio::fs::remove_file(&temp_dir_path).await?;
        }
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_filename() {
        let test_uri = "http://some_test_authority/with/path/and/query.extension";
        let test_transfer = Transfer::init(test_uri).await;
        assert_eq!(test_transfer.filename.to_str().unwrap(), "query.extension");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_path() {
        let test_filename = "some_test_filename.extension";
        let test_path = Transfer::create_path(&test_filename).await;
        assert_eq!(test_path.to_str().unwrap(), "some_test_filename.extension");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_temp_dir() -> Result<(), std::io::Error> {
        Transfer::create_temp_dir().await;
        let test_temp_dir_metadata = tokio::fs::metadata("/tmp/archeon").await?;
        assert_eq!(test_temp_dir_metadata.is_dir(), true);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn install_package() -> Result<(), std::io::Error> {
        // let f = Path::new("some_test_filename.deb");
        let test_mock_url = mockito::server_url();
        // test_mock_url.push_str("/some_test_filename.deb");
        let test_transfer = Transfer::init(&test_mock_url).await;
        // let mock = mock("GET", "/some_test_filename.deb")
        let mock = mock("GET", "/")
            .with_status(200)
            // .with_body_from_file(f)
            .with_body(b"test_body")
            .create();
        test_transfer.launch().await;
        test_transfer.install_package().await?;
        mock.assert();
        assert!(mock.matched());
        Ok(())
    }
}
