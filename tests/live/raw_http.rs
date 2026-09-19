use decay::domain::{ARTIFACT_DURATION_HEADER, ARTIFACT_TAG_HEADER};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedReadHalf;
use tokio::task::JoinHandle;

/// Task run time the tests send on every upload and read back on download.
pub const DURATION: &str = "1234";

/// Size of each HTTP chunk the raw-socket uploads write.
const CHUNK: usize = 64 * 1024;

/// Uploads with a `Content-Length` and answers the status code.
pub async fn put_fixed_length(
    client: &reqwest::Client,
    url: &str,
    tag: &str,
    body: Vec<u8>,
) -> u16 {
    client
        .put(url)
        .header("Content-Type", "application/octet-stream")
        .header(ARTIFACT_TAG_HEADER, tag)
        .header(ARTIFACT_DURATION_HEADER, DURATION)
        .body(body)
        .send()
        .await
        .expect("Failed to PUT the artifact")
        .status()
        .as_u16()
}

/// Uploads with `Transfer-Encoding: chunked` and answers the status code.
///
/// The request goes over a raw socket because the test build of reqwest cannot
/// stream a request body.
pub async fn put_chunked(socket_address: &str, path: &str, tag: &str, body: Vec<u8>) -> u16 {
    let stream = TcpStream::connect(socket_address)
        .await
        .expect("Failed to connect to the cache server");
    let (reader, mut writer) = stream.into_split();
    let head = chunked_head(socket_address, path, tag);

    let sender: JoinHandle<std::io::Result<()>> = tokio::spawn(async move {
        writer.write_all(head.as_bytes()).await?;
        for chunk in body.chunks(CHUNK) {
            write_chunk(&mut writer, chunk).await?;
        }
        writer.write_all(b"0\r\n\r\n").await
    });

    let status = read_status(reader, path).await;
    sender
        .await
        .expect("The socket writer panicked")
        .expect("Failed to write the chunked request");

    status
}

/// A chunked upload whose body never ends.
pub struct AbandonedUpload {
    sender: JoinHandle<()>,
    reader: OwnedReadHalf,
}

impl AbandonedUpload {
    /// Starts the upload and keeps the connection open.
    pub async fn start(socket_address: &str, path: &str, tag: &str, body: Vec<u8>) -> Self {
        let stream = TcpStream::connect(socket_address)
            .await
            .expect("Failed to connect to the cache server");
        let (reader, mut writer) = stream.into_split();
        let head = chunked_head(socket_address, path, tag);

        let sender = tokio::spawn(async move {
            if writer.write_all(head.as_bytes()).await.is_err() {
                return;
            }

            for chunk in body.chunks(CHUNK) {
                if write_chunk(&mut writer, chunk).await.is_err() {
                    return;
                }
            }

            // No terminating zero chunk, and the socket stays open until the
            // test drops it, so the server sees a broken body and not an EOF.
            std::future::pending::<()>().await
        });

        Self { sender, reader }
    }

    /// Drops the connection with the body unfinished.
    pub async fn disconnect(self) {
        self.sender.abort();
        let _ = self.sender.await;
        drop(self.reader);
    }
}

fn chunked_head(host: &str, path: &str, tag: &str) -> String {
    format!(
        "PUT {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Content-Type: application/octet-stream\r\n\
         Transfer-Encoding: chunked\r\n\
         {ARTIFACT_TAG_HEADER}: {tag}\r\n\
         {ARTIFACT_DURATION_HEADER}: {DURATION}\r\n\
         Connection: close\r\n\r\n"
    )
}

async fn write_chunk<W>(writer: &mut W, chunk: &[u8]) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    writer
        .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
        .await?;
    writer.write_all(chunk).await?;
    writer.write_all(b"\r\n").await
}

async fn read_status(reader: OwnedReadHalf, path: &str) -> u16 {
    let mut status_line = String::new();
    let mut buffered = BufReader::new(reader);

    tokio::time::timeout(crate::REQUEST_TIMEOUT, buffered.read_line(&mut status_line))
        .await
        .unwrap_or_else(|_| panic!("Timed out reading the response status line of {path}"))
        .unwrap_or_else(|error| {
            panic!("Failed to read the response status line of {path}: {error}")
        });

    status_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_else(|| panic!("Malformed response status line: {status_line:?}"))
        .parse()
        .unwrap_or_else(|_| panic!("Malformed response status line: {status_line:?}"))
}
