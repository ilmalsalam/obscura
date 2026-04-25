//! Rendering module for obscura browser
//!
//! This module provides HTML/CSS to PNG rendering without external Chrome dependency.

mod layout;
mod style;

pub use layout::{LayoutBox, LayoutTree, RenderContext};
pub use style::{
    AlignItems, CSSDeclaration, CSSRule, ComputedStyles, FlexDirection, FlexWrap, GridTrack,
    JustifyContent, Stylesheet,
};

/// Render the DOM tree to a PNG image and return base64-encoded data
///
/// Returns CDP-compatible screenshot response:
/// ```json
/// {
///     "data": "base64-encoded-png-data",
///     "mimeType": "image/png"
/// }
/// ```
pub fn render_screenshot(
    dom: &obscura_dom::tree::DomTree,
    width: u32,
    height: u32,
) -> Result<String, RenderError> {
    render_screenshot_with_stylesheets(dom, width, height, &[])
}

/// Render the DOM tree to a PNG image with external stylesheets and return base64-encoded data
pub fn render_screenshot_with_stylesheets(
    dom: &obscura_dom::tree::DomTree,
    width: u32,
    height: u32,
    stylesheets: &[Stylesheet],
) -> Result<String, RenderError> {
    let png_bytes = render_screenshot_bytes_with_stylesheets(dom, width, height, stylesheets)?;

    // Base64 encode
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

    Ok(encoded)
}

/// Render the DOM tree to a PNG image and return raw PNG bytes
pub fn render_screenshot_bytes(
    dom: &obscura_dom::tree::DomTree,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, RenderError> {
    render_screenshot_bytes_with_stylesheets(dom, width, height, &[])
}

/// Render the DOM tree to a PNG image with external stylesheets and return raw PNG bytes
pub fn render_screenshot_bytes_with_stylesheets(
    dom: &obscura_dom::tree::DomTree,
    width: u32,
    height: u32,
    stylesheets: &[Stylesheet],
) -> Result<Vec<u8>, RenderError> {
    let scale = 1.0;

    // Build layout tree from DOM with stylesheets
    let layout = LayoutTree::build_with_stylesheets(dom, width, height, stylesheets);

    // Create pixel buffer
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or(RenderError::ImageCreationFailed)?;

    // Fill background with white
    pixmap.fill(tiny_skia::Color::WHITE);

    // Create font system with system fonts on macOS
    let font_system = create_font_system();

    let swash_cache = cosmic_text::SwashCache::new();

    // Create render context
    let mut ctx = RenderContext {
        pixmap: &mut pixmap,
        width,
        height,
        scale,
        font_system,
        swash_cache,
    };

    // Render the layout tree
    layout.render(&mut ctx)?;

    // Encode as PNG
    let png_data = pixmap
        .encode_png()
        .map_err(|_| RenderError::ImageCreationFailed)?;

    Ok(png_data)
}

/// Create a FontSystem with system fonts loaded
#[cfg(target_os = "macos")]
fn create_font_system() -> cosmic_text::FontSystem {
    use cosmic_text::fontdb::Database;

    let mut db = Database::new();
    db.load_system_fonts();
    cosmic_text::FontSystem::new_with_locale_and_db("en-US".into(), db)
}

/// Create a FontSystem with system fonts loaded
#[cfg(not(target_os = "macos"))]
fn create_font_system() -> cosmic_text::FontSystem {
    use cosmic_text::fontdb::Database;

    let mut db = Database::new();
    db.load_system_fonts();
    cosmic_text::FontSystem::new_with_locale_and_db("en-US".into(), db)
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Failed to create image buffer")]
    ImageCreationFailed,

    #[error("Layout error: {0}")]
    LayoutError(String),

    #[error("Text rendering error: {0}")]
    TextRenderError(String),

    #[error("Font error: {0}")]
    FontError(String),
}

impl From<RenderError> for String {
    fn from(e: RenderError) -> Self {
        e.to_string()
    }
}
