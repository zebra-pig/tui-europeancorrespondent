use image::{DynamicImage, ImageReader};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::FontSize;
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use tokio::sync::mpsc;

pub struct CachedImage {
    pub source: DynamicImage,
    /// Protocol cached for a specific (width, height) in cells
    pub proto: Option<StatefulProtocol>,
    pub proto_size: (u16, u16),
}

pub struct ImageCache {
    proto_type: ProtocolType,
    font_size: FontSize,
    client: Client,
    pub images: HashMap<String, CachedImage>,
    pending: HashSet<String>,
    tx: mpsc::UnboundedSender<(String, DynamicImage)>,
}

impl ImageCache {
    pub fn new(picker: &Picker) -> (Self, mpsc::UnboundedReceiver<(String, DynamicImage)>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                proto_type: picker.protocol_type(),
                font_size: picker.font_size(),
                client: Client::new(),
                images: HashMap::new(),
                pending: HashSet::new(),
                tx,
            },
            rx,
        )
    }

    pub fn insert(&mut self, url: String, img: DynamicImage) {
        self.pending.remove(&url);
        self.images.insert(url, CachedImage {
            source: img,
            proto: None,
            proto_size: (0, 0),
        });
    }

    /// Get a StatefulProtocol for the given URL, pre-resized to cover (w, h) cells.
    /// Creates/re-creates the protocol if the size changed.
    pub fn get_cover(&mut self, url: &str, w: u16, h: u16) -> Option<&mut StatefulProtocol> {
        let font = self.font_size;
        let proto_type = self.proto_type;

        let cached = self.images.get_mut(url)?;

        // Rebuild protocol if size changed
        if cached.proto.is_none() || cached.proto_size != (w, h) {
            // object-fit: cover = resize_to_fill then crop
            // Subtract 1 cell to avoid edge artifacts from rounding
            let render_w = w.saturating_sub(1).max(1);
            let render_h = h.saturating_sub(1).max(1);
            let px_w = (render_w as u32) * (font.0 as u32);
            let px_h = (render_h as u32) * (font.1 as u32);
            if px_w == 0 || px_h == 0 { return None; }

            let covered = cached.source.resize_to_fill(
                px_w, px_h,
                image::imageops::FilterType::Triangle,
            );

            let mut picker = Picker::from_fontsize(font);
            picker.set_protocol_type(proto_type);
            cached.proto = Some(picker.new_resize_protocol(covered));
            cached.proto_size = (w, h);
        }

        cached.proto.as_mut()
    }

    pub fn fetch(&mut self, url: &str) {
        if self.pending.contains(url) || self.images.contains_key(url) {
            return;
        }
        self.pending.insert(url.to_string());

        let url = url.to_string();
        let client = self.client.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            if let Ok(img) = fetch_image(&client, &url).await {
                let _ = tx.send((url, img));
            }
        });
    }
}

async fn fetch_image(client: &Client, url: &str) -> Result<DynamicImage, String> {
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("{}", e))?
        .bytes()
        .await
        .map_err(|e| format!("{}", e))?;

    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("{}", e))?
        .decode()
        .map_err(|e| format!("{}", e))
}
