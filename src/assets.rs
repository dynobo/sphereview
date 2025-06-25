use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "resources/photosphereviewer/dist"]
pub struct Asset;
