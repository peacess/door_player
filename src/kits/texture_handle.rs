use std::sync::Arc;

use eframe::epaint::{emath::NumExt, mutex::RwLock, textures::TextureOptions, ImageData, ImageDelta, TextureId, TextureManager};

/// Used to paint images.
///
/// An _image_ is pixels stored in RAM, and represented using [`ImageData`].
/// Before you can paint it however, you need to convert it to a _texture_.
///
/// If you are using egui, use `egui::Context::load_texture`.
///
/// The [`TextureHandleNoMut`] can be cloned cheaply.
/// When the last [`TextureHandleNoMut`] for specific texture is dropped, the texture is freed.
///
/// See also [`TextureManager`].
#[must_use]
pub struct TextureHandleNoMut {
    tex_mngr: Arc<RwLock<TextureManager>>,
    id: TextureId,
}

impl Drop for TextureHandleNoMut {
    fn drop(&mut self) {
        self.tex_mngr.write().free(self.id);
    }
}

impl Clone for TextureHandleNoMut {
    fn clone(&self) -> Self {
        self.tex_mngr.write().retain(self.id);
        Self {
            tex_mngr: self.tex_mngr.clone(),
            id: self.id,
        }
    }
}

impl PartialEq for TextureHandleNoMut {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TextureHandleNoMut {}

impl std::hash::Hash for TextureHandleNoMut {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl TextureHandleNoMut {
    /// If you are using egui, use `egui::Context::load_texture` instead.
    pub fn new(tex_mngr: Arc<RwLock<TextureManager>>, id: TextureId) -> Self {
        Self { tex_mngr, id }
    }

    #[inline]
    pub fn id(&self) -> TextureId {
        self.id
    }

    /// Assign a new image to an existing texture.
    pub fn set(&self, image: impl Into<ImageData>, options: TextureOptions) {
        self.tex_mngr.write().set(self.id, ImageDelta::full(image.into(), options));
    }

    /// Assign a new image to a subregion of the whole texture.
    pub fn set_partial(&self, pos: [usize; 2], image: impl Into<ImageData>, options: TextureOptions) {
        self.tex_mngr.write().set(self.id, ImageDelta::partial(pos, image.into(), options));
    }

    /// width x height
    pub fn size(&self) -> [usize; 2] {
        self.tex_mngr.read().meta(self.id).map_or([0, 0], |tex| tex.size)
    }

    /// width x height
    pub fn size_vec2(&self) -> eframe::epaint::Vec2 {
        let [w, h] = self.size();
        eframe::epaint::Vec2::new(w as f32, h as f32)
    }

    /// `width x height x bytes_per_pixel`
    pub fn byte_size(&self) -> usize {
        self.tex_mngr.read().meta(self.id).map_or(0, |tex| tex.bytes_used())
    }

    /// width / height
    pub fn aspect_ratio(&self) -> f32 {
        let [w, h] = self.size();
        w as f32 / h.at_least(1) as f32
    }

    /// Debug-name.
    pub fn name(&self) -> String {
        self.tex_mngr.read().meta(self.id).map_or_else(|| "<none>".to_owned(), |tex| tex.name.clone())
    }
}

impl From<&TextureHandleNoMut> for TextureId {
    #[inline(always)]
    fn from(handle: &TextureHandleNoMut) -> Self {
        handle.id()
    }
}

impl From<&mut TextureHandleNoMut> for TextureId {
    #[inline(always)]
    fn from(handle: &mut TextureHandleNoMut) -> Self {
        handle.id()
    }
}
