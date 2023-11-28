#[cfg(test)]
mod tests {
    use ort::{SessionBuilder, Environment, GraphOptimizationLevel};

    #[test]
    fn test_compute_embedding_from_mel_spec() {
        const AUDIO_MODEL_RELATIVE_PATH: String = "./onnx_models/clap-htsat-unfused_audio_with_projection.onnx".to_owned();
        let environment = Environment::builder()
            .with_execution_providers(vec![
                CUDA(Default::default()),
                CoreML(Default::default()),
                CPU(Default::default()),
            ])
            .with_name("CLAP")
            .build()?
            .into_arc();
        let audio_model = SessionBuilder::new(&environment)?
        .with_optimization_level(GraphOptimizationLevel::Disable)?
        .with_model_from_file(AUDIO_MODEL_RELATIVE_PATH)
        .unwrap_or_else(|_| {
            panic!(
                "Failed to load audio embedder model from {}",
                text_embedder_model_filename
            )
        });

        println!(Session)
    }

    // Additional helper functions to create dummy inputs, load models, etc.
    // ...
}
