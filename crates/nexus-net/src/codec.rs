//! Bounded length-prefixed framing for `/nexus/test/1`, `/nexus/file/1`, and
//! `/nexus/index/1`.
//!
//! The request-response behaviour does not frame messages for us, so this
//! codec writes a four-byte big-endian length followed by the canonical CBOR
//! body. Both directions reject empty, truncated, and oversized frames before
//! allocating.

use std::io;

use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::request_response::Codec;
use libp2p::swarm::StreamProtocol;
use nexus_protocol::{MAX_FILE_MESSAGE_BYTES, MAX_INDEX_MESSAGE_BYTES, MAX_TEST_MESSAGE_BYTES};

/// Protocol ID of the Stage 09 diagnostic request/response protocol.
pub const TEST_PROTOCOL_NAME: &str = "/nexus/test/1";
/// Protocol ID of the Stage 12 file metadata and chunk protocol.
pub const FILE_PROTOCOL_NAME: &str = "/nexus/file/1";
/// Protocol ID of the Stage 16 optional index-provider protocol.
pub const INDEX_PROTOCOL_NAME: &str = "/nexus/index/1";

const LENGTH_PREFIX_BYTES: usize = 4;

/// Bounded request/response codec carrying canonical CBOR bodies.
#[derive(Clone, Copy, Debug, Default)]
pub struct TestCodec;

impl Codec for TestCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_frame(io).await
    }

    async fn read_response<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_frame(io).await
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        request: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_frame(io, &request).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        response: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_frame(io, &response).await
    }
}

/// Returns the request-response stream protocol.
#[must_use]
pub fn test_protocol() -> StreamProtocol {
    StreamProtocol::new(TEST_PROTOCOL_NAME)
}

/// Returns the file-transfer stream protocol.
#[must_use]
pub fn file_protocol() -> StreamProtocol {
    StreamProtocol::new(FILE_PROTOCOL_NAME)
}

/// Returns the optional index-provider stream protocol.
#[must_use]
pub fn index_protocol() -> StreamProtocol {
    StreamProtocol::new(INDEX_PROTOCOL_NAME)
}

/// Bounded request/response codec carrying canonical CBOR index messages.
#[derive(Clone, Copy, Debug, Default)]
pub struct IndexCodec;

impl Codec for IndexCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_frame_with(io, MAX_INDEX_MESSAGE_BYTES, "index").await
    }

    async fn read_response<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_frame_with(io, MAX_INDEX_MESSAGE_BYTES, "index").await
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        request: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_frame_with(io, &request, MAX_INDEX_MESSAGE_BYTES, "index").await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        response: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_frame_with(io, &response, MAX_INDEX_MESSAGE_BYTES, "index").await
    }
}

/// Bounded request/response codec carrying canonical CBOR file messages.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileCodec;

impl Codec for FileCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_frame_with(io, MAX_FILE_MESSAGE_BYTES, "file").await
    }

    async fn read_response<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_frame_with(io, MAX_FILE_MESSAGE_BYTES, "file").await
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        request: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_frame_with(io, &request, MAX_FILE_MESSAGE_BYTES, "file").await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        response: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_frame_with(io, &response, MAX_FILE_MESSAGE_BYTES, "file").await
    }
}

async fn read_frame<T>(io: &mut T) -> io::Result<Vec<u8>>
where
    T: AsyncRead + Unpin + Send,
{
    read_frame_with(io, MAX_TEST_MESSAGE_BYTES, "transport-check").await
}

async fn write_frame<T>(io: &mut T, body: &[u8]) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    write_frame_with(io, body, MAX_TEST_MESSAGE_BYTES, "transport-check").await
}

async fn read_frame_with<T>(io: &mut T, max: usize, label: &'static str) -> io::Result<Vec<u8>>
where
    T: AsyncRead + Unpin + Send,
{
    let mut prefix = [0_u8; LENGTH_PREFIX_BYTES];
    io.read_exact(&mut prefix).await?;
    let length = usize::try_from(u32::from_be_bytes(prefix)).map_err(|_| invalid_data(label))?;
    if length == 0 || length > max {
        return Err(invalid_data(label));
    }
    let mut body = vec![0_u8; length];
    io.read_exact(&mut body).await?;
    Ok(body)
}

async fn write_frame_with<T>(io: &mut T, body: &[u8], max: usize, label: &'static str) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    if body.is_empty() || body.len() > max {
        return Err(invalid_data(label));
    }
    let length = u32::try_from(body.len()).map_err(|_| invalid_data(label))?;
    io.write_all(&length.to_be_bytes()).await?;
    io.write_all(body).await?;
    io.flush().await?;
    Ok(())
}

fn invalid_data(label: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{label} frame is out of bounds"))
}
