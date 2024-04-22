use axum::{extract::Multipart, http::StatusCode};
use fxhash::FxHashMap;
use image::{DynamicImage, ImageFormat};

use crate::Error;

pub struct ImageFileBuilder {
    pub fields: FxHashMap<String, String>,
    pub image: Option<(DynamicImage, ImageFormat)>,
}

impl ImageFileBuilder {
    pub async fn build(mut multipart: Multipart) -> Result<Self, Error> {
        let mut builder = ImageFileBuilder {
            fields: FxHashMap::default(),
            image: None,
        };

        while let Some(field) = multipart.next_field().await? {
            if let Some(file_name) = field.file_name() {
                if file_name == "" {
                    continue;
                }

                let (_name, ext) = file_name.split_once(".").ok_or(Error::Custom {
                    status_code: StatusCode::BAD_REQUEST,
                    error: format!("File name: {file_name} has no extension"),
                })?;
                let format = ImageFormat::from_extension(ext).ok_or(Error::Custom {
                    status_code: StatusCode::BAD_REQUEST,
                    error: format!("Could not figure out image format from extension: {ext}"),
                })?;

                let img_bytes = field.bytes().await?.to_vec();
                let image = image::load_from_memory_with_format(&img_bytes, format)?;

                builder.image = Some((image, format));
            } else if let Some(name) = field.name() {
                builder.fields.insert(name.into(), field.text().await?);
            }
        }

        Ok(builder)
    }

    pub fn missing_fields(&self, fields: &[&'static str]) -> Vec<&'static str> {
        let mut missing = Vec::default();
        for needed in fields {
            if !self.fields.contains_key(*needed) {
                missing.push(*needed);
            }
        }
        missing
    }
}
