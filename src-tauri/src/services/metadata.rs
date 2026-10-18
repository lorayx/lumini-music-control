use base64::Engine;
use windows::{Media::Control::GlobalSystemMediaTransportControlsSession, Storage::Streams::DataReader};
use windows_future::IAsyncOperation;
use windows::core::RuntimeType;

pub async fn wait<T: RuntimeType + 'static>(op: IAsyncOperation<T>) -> Result<T, String> {
    match tokio::time::timeout(std::time::Duration::from_secs(8), op.clone()).await {
        Ok(result) => result.map_err(|e| e.to_string()),
        Err(_) => { let _ = op.Cancel(); Err("Windows не ответила за 8 секунд".into()) }
    }
}

pub struct Metadata { pub title: String, pub artist: String, pub album: String, pub cover: Option<String>, pub warning: Option<String> }
pub async fn read(session: &GlobalSystemMediaTransportControlsSession) -> Result<Metadata, String> {
    let p = wait(session.TryGetMediaPropertiesAsync().map_err(|e| e.to_string())?).await?;
    let mut data = Metadata { title: p.Title().unwrap_or_default().to_string(), artist: p.Artist().unwrap_or_default().to_string(), album: p.AlbumTitle().unwrap_or_default().to_string(), cover: None, warning: None };
    if let Ok(reference) = p.Thumbnail() {
        let cover_result: Result<Option<String>,String> = async {
        let stream = wait(reference.OpenReadAsync().map_err(|e| e.to_string())?).await?;
        let result = async {
            let size = stream.Size().map_err(|e| e.to_string())?;
            if size == 0 || size > 5 * 1024 * 1024 { return Ok(None); }
            let input = stream.GetInputStreamAt(0).map_err(|e| e.to_string())?;
            let reader = DataReader::CreateDataReader(&input).map_err(|e| e.to_string())?;
            let result = async {
                let operation: IAsyncOperation<u32> = reader.LoadAsync(size as u32).map_err(|e| e.to_string())?.cast().map_err(|e| e.to_string())?;
                let loaded = wait(operation).await?;
                if loaded != size as u32 { return Err("Неполная обложка".into()); }
                let mut bytes = vec![0; size as usize]; reader.ReadBytes(&mut bytes).map_err(|e| e.to_string())?;
                let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { "image/png" } else if bytes.starts_with(&[0xff,0xd8,0xff]) { "image/jpeg" } else { return Ok(None); };
                Ok(Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))))
            }.await;
            let _ = reader.Close(); let _ = input.Close(); result
        }.await;
        let _ = stream.Close(); result
        }.await;
        match cover_result { Ok(cover) => data.cover = cover, Err(error) => data.warning = Some(error) }
    }
    Ok(data)
}
use windows::core::Interface;
