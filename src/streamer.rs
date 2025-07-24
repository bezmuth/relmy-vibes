use std::num::NonZeroUsize;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use stream_download::http::HttpStream;
use stream_download::http::reqwest::Client;
use stream_download::source::{DecodeError, SourceStream};
use stream_download::storage::bounded::BoundedStorageProvider;
use stream_download::storage::memory::MemoryStorageProvider;
use stream_download::{Settings, StreamDownload};
use tokio_util::sync::CancellationToken;

use crate::Error;

pub async fn play(
    url: String,
    volume: Arc<RwLock<f32>>,
    token: CancellationToken,
) -> Result<(), Error> {
    let stream = HttpStream::<Client>::create(url.parse().unwrap())
        .await
        .unwrap();

    let bitrate: u64 = stream.header("Icy-Br").unwrap().parse().unwrap();

    // buffer 2 seconds of audio
    // bitrate (in kilobits) / bits per byte * bytes per kilobyte * 2 seconds
    let prefetch_bytes = bitrate / 8 * 1024 * 2;

    let reader = match StreamDownload::from_stream(
        stream,
        // use bounded storage to keep the underlying size from growing indefinitely
        BoundedStorageProvider::new(
            MemoryStorageProvider,
            // be liberal with the buffer size, you need to make sure it holds enough space to
            // prevent any out-of-bounds reads
            NonZeroUsize::new(512 * 1024).unwrap(),
        ),
        Settings::default().prefetch_bytes(prefetch_bytes),
    )
    .await
    {
        Ok(reader) => reader,
        Err(e) => panic!("{:?}", e.decode_error().await),
    };

    let handle = tokio::spawn(async move {
        let (_stream, handle) = rodio::OutputStream::try_default().unwrap();
        let sink = rodio::Sink::try_new(&handle).unwrap();
        sink.append(rodio::Decoder::new(reader).unwrap());
        loop {
            sink.set_volume(*volume.read().unwrap());
            if token.is_cancelled() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        sink.stop();
    });

    handle.await.unwrap();
    Ok(())
}
