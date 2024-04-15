use std::collections::HashMap;

use qdrant_client::{client::QdrantClient, qdrant};
use uuid::Uuid;

use crate::{Error, Platform};

// TODO: placeholder struct
struct EmbeddingEncoder;

impl EmbeddingEncoder {
    pub async fn new() -> Result<Self, Error> {
        todo!()
    }

    pub async fn encode(&'static self, text: String) -> Result<Vec<f32>, Error> {
        todo!()
    }
}

pub struct EmbeddingDb {
    client: QdrantClient,
    encoder: EmbeddingEncoder,
}

impl EmbeddingDb {
    const CREATOR_COLLECTION_NAME: &'static str = "creator-collection";
    const SPONSOR_COLLECTION_NAME: &'static str = "sponsor-collection";

    pub async fn new(url: &str) -> Result<Self, Error> {
        let encoder = EmbeddingEncoder::new().await?;

        let client = QdrantClient::from_url(url)
            .build()
            .map_err(Error::QdrantError)?;

        if !client
            .collection_exists(Self::CREATOR_COLLECTION_NAME)
            .await
            .map_err(Error::QdrantError)?
        {
            client
                .create_collection(&qdrant::CreateCollection {
                    collection_name: Self::CREATOR_COLLECTION_NAME.into(),
                    vectors_config: Some(qdrant::VectorsConfig {
                        config: Some(qdrant::vectors_config::Config::Params(
                            qdrant::VectorParams {
                                size: 1024,
                                distance: qdrant::Distance::Dot as i32,
                                ..Default::default()
                            },
                        )),
                    }),
                    ..Default::default()
                })
                .await
                .map_err(Error::QdrantError)?;
        }

        Ok(EmbeddingDb { client, encoder })
    }

    fn format_creator_descriptions(profile: &str, content: &str, audience: &str) -> String {
        format!("### Content Creator Profile Description:\n{profile}\n\n### Content Creator Content Description:\n{content}\n\n### Content Creator Audience Description:\n{audience}")
    }

    fn format_sponsor_descriptions(profile: &str, product: &str) -> String {
        format!("### Sponsor Profile Description:\n{profile}\n\n### Sponsor Product Description:\n{product}")
    }

    pub async fn insert_update_creator(
        &'static self,
        user_id: Uuid,
        profile_desc: &str,
        content_desc: &str,
        audience_desc: &str,
        platforms: Vec<Platform>,
    ) -> Result<(), Error> {
        let mut payload = HashMap::default();
        payload.insert(
            "platforms".into(),
            serde_json::value::to_value(&platforms)?
                .try_into()
                .expect("Failed to convert serde_json::Value to qdrant::Value"),
        );

        let user_desc =
            Self::format_creator_descriptions(profile_desc, content_desc, audience_desc);
        let vectors = self.encoder.encode(user_desc).await?;

        let point = qdrant::PointStruct {
            id: Some(user_id.to_string().into()),
            payload,
            vectors: Some(vectors.into()),
        };
        self.client
            .upsert_points(Self::CREATOR_COLLECTION_NAME, None, vec![point], None)
            .await
            .map_err(Error::QdrantError)?;

        Ok(())
    }

    pub async fn overwrite_creator_platforms(
        &'static self,
        user_id: Uuid,
        platforms: Vec<Platform>,
    ) -> Result<(), Error> {
        let mut payload = HashMap::default();
        payload.insert(
            "platforms".into(),
            serde_json::value::to_value(&platforms)?
                .try_into()
                .expect("Failed to convert serde_json::Value to qdrant::Value"),
        );

        self.client
            .overwrite_payload(
                Self::CREATOR_COLLECTION_NAME,
                None,
                &qdrant::PointsSelector {
                    points_selector_one_of: Some(
                        qdrant::points_selector::PointsSelectorOneOf::Points(
                            qdrant::PointsIdsList {
                                ids: vec![user_id.to_string().into()],
                            },
                        ),
                    ),
                },
                payload.into(),
                None,
                None,
            )
            .await
            .map_err(Error::QdrantError)?;

        Ok(())
    }

    pub async fn update_creator_desc(
        &'static self,
        user_id: Uuid,
        profile_desc: &str,
        content_desc: &str,
        audience_desc: &str,
    ) -> Result<(), Error> {
        let user_desc =
            Self::format_creator_descriptions(profile_desc, content_desc, audience_desc);
        let vectors = self.encoder.encode(user_desc).await?;

        self.client
            .update_vectors(
                Self::CREATOR_COLLECTION_NAME,
                None,
                &[qdrant::PointVectors {
                    id: Some(user_id.to_string().into()),
                    vectors: Some(vectors.into()),
                }],
                None,
            )
            .await
            .map_err(Error::QdrantError)?;

        Ok(())
    }

    pub async fn insert_update_sponsor(
        &'static self,
        company_id: Uuid,
        profile_desc: &str,
        product_desc: &str,
    ) -> Result<(), Error> {
        let user_desc = Self::format_sponsor_descriptions(profile_desc, product_desc);
        let vectors = self.encoder.encode(user_desc).await?;

        let point = qdrant::PointStruct {
            id: Some(company_id.to_string().into()),
            payload: HashMap::default(),
            vectors: Some(vectors.into()),
        };
        self.client
            .upsert_points(Self::CREATOR_COLLECTION_NAME, None, vec![point], None)
            .await
            .map_err(Error::QdrantError)?;

        Ok(())
    }
}
