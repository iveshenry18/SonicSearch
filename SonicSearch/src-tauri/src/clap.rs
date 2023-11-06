use anyhow::Result;
use ort::{
    Environment,
    ExecutionProvider::{CoreML, CPU, CUDA},
    GraphOptimizationLevel, Session, SessionBuilder,
};
use tauri::PathResolver;

pub fn load_clap_model(path_resolver: &PathResolver) -> Result<Session> {
    let environment = Environment::builder()
        .with_execution_providers(vec![
            CUDA(Default::default()),
            CoreML(Default::default()),
            CPU(Default::default()),
        ])
        .with_name("CLAP")
        .build()?
        .into_arc();
    let model_path = path_resolver.resolve_resource("onnx_models/laion_clap_htsat_unfused.onnx").expect("Model path should resolve");

    let session = SessionBuilder::new(&environment)?
        .with_optimization_level(GraphOptimizationLevel::Disable)?
        .with_model_from_file(model_path)?;

    Ok(session)
}
