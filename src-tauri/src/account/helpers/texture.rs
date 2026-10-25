use image::RgbaImage;
use sjmcl_types::error::SJMCLResult;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

use crate::account::models::{
  AccountError, PlayerCapeInfo, PlayerInfo, PresetRole, SkinModel, Texture, TextureType,
};
use crate::utils::fs::get_app_resource_filepath;
use crate::utils::image::ImageWrapper;

pub fn draw_avatar(size: u32, img: &RgbaImage) -> Vec<ImageWrapper> {
  let (skin_width, _) = img.dimensions();

  let scale = skin_width as f32 / 64.0;
  let face_offset = ((size as f32 / 18.0).round()) as u32;

  let mut avatar = vec![RgbaImage::new(size, size), RgbaImage::new(size, size)];

  // Draw face
  draw_image_section(
    img,
    &[8.0 * scale, 8.0 * scale, 8.0 * scale, 8.0 * scale],
    &[
      face_offset,
      face_offset,
      size - 2 * face_offset,
      size - 2 * face_offset,
    ],
    &mut avatar[0],
  );

  // Draw hat
  draw_image_section(
    img,
    &[40.0 * scale, 8.0 * scale, 8.0 * scale, 8.0 * scale],
    &[0, 0, size, size],
    &mut avatar[1],
  );

  avatar.into_iter().map(ImageWrapper::from).collect()
}

fn draw_image_section(
  img: &RgbaImage,
  src_rect: &[f32; 4],
  dest_rect: &[u32; 4],
  avatar_img: &mut RgbaImage,
) {
  let x_scale = src_rect[2] / dest_rect[2] as f32;
  let y_scale = src_rect[3] / dest_rect[3] as f32;
  for x_offset in 0..dest_rect[2] {
    for y_offset in 0..dest_rect[3] {
      let pixel = img.get_pixel(
        (src_rect[0] + x_offset as f32 * x_scale) as u32,
        (src_rect[1] + y_offset as f32 * y_scale) as u32,
      );
      if pixel.0[3] == 0 {
        continue;
      }
      avatar_img.put_pixel(dest_rect[0] + x_offset, dest_rect[1] + y_offset, *pixel);
    }
  }
}

/// Resolve the bundled preset skin file of the given preset role, together with its skin model.
pub fn load_preset_skin_info(
  app: &AppHandle,
  preset_role: PresetRole,
) -> SJMCLResult<(PathBuf, SkinModel)> {
  let skin_path = get_app_resource_filepath(app, &format!("assets/skins/{}.png", preset_role))
    .map_err(|_| AccountError::TextureError)?;
  let skin_model = match preset_role {
    PresetRole::Alex => SkinModel::Slim,
    _ => SkinModel::Default,
  };
  Ok((skin_path, skin_model))
}

/// Replace the stored texture of the same type, or append it when the player does not have one
/// yet. Other textures (for example the cape while the skin changes) are left untouched.
pub fn upsert_texture(textures: &[Texture], new_texture: Texture) -> Vec<Texture> {
  let mut textures = textures.to_vec();
  if let Some(existing) = textures
    .iter_mut()
    .find(|texture| texture.texture_type == new_texture.texture_type)
  {
    *existing = new_texture;
  } else {
    textures.push(new_texture);
  }
  textures
}

/// Describe the locally stored cape of an offline or third-party player.
///
/// Those account types have no Mojang cape list: the cape they uploaded to their own auth server
/// is the only one the launcher knows about.
pub fn local_cape_info(textures: &[Texture]) -> Option<PlayerCapeInfo> {
  textures
    .iter()
    .find(|texture| texture.texture_type == TextureType::Cape)
    .map(|texture| PlayerCapeInfo {
      id: String::new(),
      alias: String::new(),
      image: texture.image.clone(),
      active: true,
    })
}

/// Operations on the textures (skin and cape) of a remote player profile.
///
/// Each account type implements this trait for the profile model its API returns, so that the
/// texture commands can be written once for every account type.
pub trait TextureOperation<T> {
  async fn parse_skin(app: &AppHandle, profile: &T) -> SJMCLResult<Vec<Texture>>;
  async fn upload_skin(
    app: &AppHandle,
    player: &PlayerInfo,
    file_path: &Path,
    model: SkinModel,
  ) -> SJMCLResult<T>;
  async fn upload_cape(app: &AppHandle, player: &PlayerInfo, file_path: &Path) -> SJMCLResult<T>;
  async fn delete_cape(app: &AppHandle, player: &PlayerInfo) -> SJMCLResult<T>;
}

#[cfg(test)]
mod tests {
  use crate::account::helpers::texture::{local_cape_info, upsert_texture};
  use crate::account::models::{PlayerCapeInfo, PresetRole, SkinModel, Texture, TextureType};
  use crate::utils::image::ImageWrapper;
  use image::{Rgba, RgbaImage};

  fn texture(texture_type: TextureType, pixel: [u8; 4], preset: Option<PresetRole>) -> Texture {
    Texture {
      texture_type,
      image: ImageWrapper::from(RgbaImage::from_pixel(2, 2, Rgba(pixel))),
      model: SkinModel::Default,
      preset,
    }
  }

  #[test]
  fn upsert_texture_replaces_skin_and_keeps_cape() {
    let cape = texture(TextureType::Cape, [1, 2, 3, 255], None);
    let stored_skin = texture(TextureType::Skin, [4, 5, 6, 255], None);
    let new_skin = texture(TextureType::Skin, [7, 8, 9, 255], Some(PresetRole::Alex));

    let textures = upsert_texture(&[stored_skin, cape.clone()], new_skin.clone());

    assert_eq!(textures.len(), 2);
    assert_eq!(textures[0], new_skin);
    assert_eq!(textures[1], cape);
  }

  #[test]
  fn upsert_texture_appends_skin_when_missing() {
    let cape = texture(TextureType::Cape, [1, 2, 3, 255], None);
    let new_skin = texture(TextureType::Skin, [7, 8, 9, 255], Some(PresetRole::Steve));

    let textures = upsert_texture(std::slice::from_ref(&cape), new_skin.clone());

    assert_eq!(textures, vec![cape, new_skin]);
  }

  #[test]
  fn local_cape_info_uses_the_stored_cape() {
    let cape = texture(TextureType::Cape, [1, 2, 3, 255], None);
    let skin = texture(TextureType::Skin, [4, 5, 6, 255], None);

    assert_eq!(
      local_cape_info(&[skin, cape.clone()]),
      Some(PlayerCapeInfo {
        id: String::new(),
        alias: String::new(),
        image: cape.image.clone(),
        active: true,
      })
    );
  }

  #[test]
  fn local_cape_info_is_empty_without_a_stored_cape() {
    let skin = texture(TextureType::Skin, [4, 5, 6, 255], None);

    assert_eq!(local_cape_info(&[skin]), None);
  }
}
