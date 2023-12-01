use anyhow::Result;
use ort::{
    Environment,
    ExecutionProvider::{CoreML, CPU, CUDA},
    GraphOptimizationLevel, Session, SessionBuilder,
};
use tauri::PathResolver;

pub fn load_clap_models(path_resolver: &PathResolver) -> Result<(Session, Session)> {
    let environment = Environment::builder()
        .with_execution_providers(vec![
            CUDA(Default::default()),
            CoreML(Default::default()),
            CPU(Default::default()),
        ])
        .with_name("CLAP")
        .build()?
        .into_arc();

    let text_embedder_model_filename = "onnx_models/clap-htsat-unfused_text_with_projection.onnx";
    let text_embedder_model_path = path_resolver
        .resolve_resource(text_embedder_model_filename)
        .unwrap_or_else(|| {
            panic!(
                "Model path {} should resolve.",
                text_embedder_model_filename
            )
        });
    let text_embedder_session = SessionBuilder::new(&environment)?
        .with_optimization_level(GraphOptimizationLevel::Disable)?
        .with_model_from_file(text_embedder_model_path)
        .unwrap_or_else(|_| {
            panic!(
                "Failed to load text embedder model from {}",
                text_embedder_model_filename
            )
        });

    let audio_embedder_model_filename = "onnx_models/clap-htsat-unfused_audio_with_projection.onnx";
    let audio_embedder_model_path = path_resolver
        .resolve_resource(audio_embedder_model_filename)
        .unwrap_or_else(|| {
            panic!(
                "Model path {} should resolve.",
                audio_embedder_model_filename
            )
        });
    let audio_embedder_session = SessionBuilder::new(&environment)?
        .with_optimization_level(GraphOptimizationLevel::Disable)?
        .with_model_from_file(audio_embedder_model_path)
        .unwrap_or_else(|_| {
            panic!(
                "Failed to load audio embedder model from {}",
                audio_embedder_model_filename
            )
        });

    Ok((text_embedder_session, audio_embedder_session))
}
