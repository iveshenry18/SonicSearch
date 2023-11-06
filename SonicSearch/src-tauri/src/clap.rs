use anyhow::Result;
use hf_hub::api::sync::Api;
use tch::CModule;

pub fn load_clap_model() -> Result<tch::CModule> {
    let api = Api::new().unwrap();
    let repo = api.model("lukewys/laion_clap".to_string());
    let model_file = repo.download("music_audioset_epoch_15_esc_90.14.pt").unwrap();

    let model = CModule::load(model_file).unwrap();

    Ok(model)
}