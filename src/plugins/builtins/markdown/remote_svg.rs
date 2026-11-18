//! Pictures of the web that are SVG without their address saying so: the badges of a README
//! (`https://img.shields.io/badge/…`) have no `.svg` at the end, and the loaders of egui go by
//! the end of the address. This one looks at what the server answered.

use eframe::egui::{
    self, ColorImage,
    load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint},
};
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
};

/// The side of the largest picture drawn, in pixels.
const MAX_SIDE: f32 = 4096.0;

/// The fonts of the computer are listed once: the text of a badge is drawn with them.
static OPTIONS: LazyLock<resvg::usvg::Options<'static>> = LazyLock::new(|| {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    options
});

type Drawn = Result<Arc<ColorImage>, String>;

#[derive(Default)]
pub struct RemoteSvg {
    cache: Mutex<HashMap<(String, SizeHint), Drawn>>,
}

/// An address of the web the SVG loader of egui does not take.
fn handles(uri: &str) -> bool {
    (uri.starts_with("https://") || uri.starts_with("http://")) && !uri.ends_with(".svg")
}

/// Whether what a server answered is an SVG picture: it says so, or the text begins as one.
fn is_svg(mime: Option<&str>, bytes: &[u8]) -> bool {
    if let Some(mime) = mime {
        return mime.contains("svg");
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    head.starts_with("<svg") || (head.starts_with("<?xml") && head.contains("<svg"))
}

/// Draws an SVG at the size asked.
fn rasterize(bytes: &[u8], size_hint: SizeHint) -> Result<ColorImage, String> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(bytes, &OPTIONS).map_err(|e| e.to_string())?;
    let (width, height) = (tree.size().width(), tree.size().height());
    let scale = match size_hint {
        SizeHint::Scale(scale) => scale.into_inner(),
        SizeHint::Width(w) => w as f32 / width,
        SizeHint::Height(h) => h as f32 / height,
        SizeHint::Size {
            width: w,
            height: h,
            ..
        } => (w as f32 / width).min(h as f32 / height),
    };
    let scale = scale.min(MAX_SIDE / width.max(height)).max(0.01);
    let (w, h) = (
        (width * scale).round().max(1.0) as u32,
        (height * scale).round().max(1.0) as u32,
    );
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("picture too large")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(w as f32 / width, h as f32 / height),
        &mut pixmap.as_mut(),
    );
    Ok(ColorImage::from_rgba_premultiplied(
        [w as usize, h as usize],
        pixmap.data(),
    ))
}

impl ImageLoader for RemoteSvg {
    fn id(&self) -> &str {
        "atelier::RemoteSvg"
    }

    fn load(&self, ctx: &egui::Context, uri: &str, size_hint: SizeHint) -> ImageLoadResult {
        if !handles(uri) {
            return Err(LoadError::NotSupported);
        }
        let key = (uri.to_string(), size_hint);
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(drawn) = cache.get(&key) {
            return match drawn {
                Ok(image) => Ok(ImagePoll::Ready {
                    image: image.clone(),
                }),
                Err(e) => Err(LoadError::Loading(e.clone())),
            };
        }
        match ctx.try_load_bytes(uri)? {
            BytesPoll::Pending { size } => Ok(ImagePoll::Pending { size }),
            BytesPoll::Ready { bytes, mime, .. } => {
                // A PNG, a JPEG: the other loaders know them.
                if !is_svg(mime.as_deref(), &bytes) {
                    return Err(LoadError::NotSupported);
                }
                let drawn = rasterize(&bytes, size_hint).map(Arc::new);
                cache.insert(key, drawn.clone());
                match drawn {
                    Ok(image) => Ok(ImagePoll::Ready { image }),
                    Err(e) => Err(LoadError::Loading(e)),
                }
            }
        }
    }

    fn forget(&self, uri: &str) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.retain(|(key, _), _| key != uri);
    }

    fn forget_all(&self) {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    fn byte_size(&self) -> usize {
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .values()
            .map(|drawn| match drawn {
                Ok(image) => image.pixels.len() * size_of::<egui::Color32>(),
                Err(e) => e.len(),
            })
            .sum()
    }
}

#[cfg(test)]
#[path = "../../../../test/plugins/builtins/markdown/remote_svg/tests.rs"]
mod tests;
