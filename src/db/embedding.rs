use axum::http::{HeaderMap, StatusCode};
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, HiddenAct, DTYPE};
use hf_hub::{api::tokio::Api, Repo};
use tokenizers::Tokenizer;

use crate::Error;

pub enum EmbeddingEncoder {
    Voyage {
        client: reqwest::Client,
    },
    Model {
        device: Device,
        model: BertModel,
        tokenizer: Tokenizer,
    },
}

impl EmbeddingEncoder {
    pub async fn new_voyage() -> Result<Self, Error> {
        let key = dotenvy::var("VOYAGE_API_KEY").map_err(|_| Error::Custom {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            error: "Missing enviorment variable VOYAGE_API_KEY".into(),
        })?;

        let mut headers = HeaderMap::default();
        headers.append(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_str(&format!("Bearer {key}"))
                .expect("Failed to create the bearer header"),
        );

        let client = reqwest::ClientBuilder::new()
            .default_headers(headers)
            .build()?;

        Ok(EmbeddingEncoder::Voyage { client })
    }

    pub async fn new_model() -> Result<Self, Error> {
        const MODEL: &'static str = "BAAI/bge-m3";

        #[cfg(feature = "cuda")]
        let device = if candle_core::utils::cuda_is_available() {
            Device::new_cuda(0)?
        } else {
            Device::Cpu
        };
        #[cfg(not(feature = "cuda"))]
        let device = Device::Cpu;

        let repo = Repo::model(MODEL.into());

        let api = Api::new()?;
        let api = api.repo(repo);

        let config = std::fs::read_to_string(api.get("config.json").await?)?;
        let mut config: Config = serde_json::from_str(&config)?;
        let tokenizer = Tokenizer::from_file(api.get("tokenizer.json").await?)
            .map_err(Error::TokenizerError)?;

        config.hidden_act = HiddenAct::GeluApproximate;

        let weights = api.get("pytorch_model.bin").await?;
        let vb = VarBuilder::from_pth(&weights, DTYPE, &device)?;

        let model = BertModel::load(vb, &config)?;

        Ok(EmbeddingEncoder::Model {
            device,
            model,
            tokenizer,
        })
    }

    pub async fn encode(&'static self, text: String) -> Result<Vec<f32>, Error> {
        match self {
            EmbeddingEncoder::Voyage { client } => {
                let req = client
                    .post("https://api.voyageai.com/v1/embeddings")
                    .json(&serde_json::json!({
                        "input": [
                            text,
                        ],
                        "model": "voyage-large-2",
                    }))
                    .build()?;

                #[derive(serde::Deserialize)]
                #[serde(tag = "object")]
                #[serde(rename_all = "lowercase")]
                enum Response {
                    List { data: Vec<Response> },
                    Embedding { embedding: Vec<f32> },
                }

                let resp: Response = client.execute(req).await?.json().await?;

                match resp {
                    Response::List { mut data } => {
                        if let Some(embedding) = data.pop() {
                            match embedding {
                                Response::List { .. } => Err(Error::Custom {
                                    status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                    error: "Voyage returned data inside data instead of embedding"
                                        .into(),
                                }),
                                Response::Embedding { embedding } => Ok(embedding),
                            }
                        } else {
                            Err(Error::Custom {
                                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                error: "No data in response from voyage".into(),
                            })
                        }
                    }
                    Response::Embedding { embedding } => Ok(embedding),
                }
            }
            EmbeddingEncoder::Model {
                device,
                model,
                tokenizer,
            } => {
                let encoded = tokio::task::spawn_blocking::<_, Result<_, Error>>(move || {
                    let tokens = tokenizer
                        .encode(text, true)
                        .map_err(Error::TokenizerError)?
                        .get_ids()
                        .to_vec();
                    let token_ids = Tensor::new(&tokens[..], &device)?.unsqueeze(0)?;
                    let token_type_ids = token_ids.zeros_like()?;

                    let ys = model.forward(&token_ids, &token_type_ids)?;
                    let (_n_sentence, n_tokens, _hidden_size) = ys.dims3()?;
                    let ys = (ys.sum(1)? / (n_tokens as f64))?;

                    Ok(ys.get(0)?.to_vec1()?)
                })
                .await??;

                Ok(encoded)
            }
        }
    }
}
