use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, HiddenAct, DTYPE};
use hf_hub::{api::tokio::Api, Repo};
use tokenizers::Tokenizer;

use crate::Error;

pub struct EmbeddingEncoder {
    device: Device,
    model: BertModel,
    tokenizer: Tokenizer,
}

impl EmbeddingEncoder {
    const MODEL: &'static str = "BAAI/bge-m3";

    pub async fn new() -> Result<Self, Error> {
        #[cfg(feature = "cuda")]
        let device = if candle_core::utils::cuda_is_available() {
            Device::new_cuda(0)?
        } else {
            Device::Cpu
        };
        #[cfg(not(feature = "cuda"))]
        let device = Device::Cpu;

        let repo = Repo::model(Self::MODEL.into());

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

        Ok(Self {
            device,
            model,
            tokenizer,
        })
    }

    pub async fn encode(&'static self, text: String) -> Result<Vec<f32>, Error> {
        let encoded = tokio::task::spawn_blocking::<_, Result<_, Error>>(|| {
            let tokens = self
                .tokenizer
                .encode(text, true)
                .map_err(Error::TokenizerError)?
                .get_ids()
                .to_vec();
            let token_ids = Tensor::new(&tokens[..], &self.device)?.unsqueeze(0)?;
            let token_type_ids = token_ids.zeros_like()?;

            let ys = self.model.forward(&token_ids, &token_type_ids)?;
            let (_n_sentence, n_tokens, _hidden_size) = ys.dims3()?;
            let ys = (ys.sum(1)? / (n_tokens as f64))?;

            Ok(ys.get(0)?.to_vec1()?)
        })
        .await??;

        Ok(encoded)
    }
}
