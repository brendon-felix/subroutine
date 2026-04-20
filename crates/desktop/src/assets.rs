use anyhow::Result;
use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "icons/**/*.svg"]
#[include = "images/**/*.png"]
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Self::get(path).map(|f| Some(f.data)).ok_or_else(|| {
            println!("Asset not found: {}", path);
            anyhow::anyhow!("Asset not found: {}", path)
        })
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| {
                if p.starts_with(path) {
                    Some(p.into())
                } else {
                    None
                }
            })
            .collect())
    }
}
