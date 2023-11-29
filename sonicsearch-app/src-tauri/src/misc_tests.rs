#[cfg(test)]
mod tests {
    use ndarray::{Array2, ArrayBase, OwnedRepr, Axis, Dim};

    // #[test]
    // fn test_compute_embedding_from_mel_spec() {
    //     const AUDIO_MODEL_RELATIVE_PATH: String =
    //         "./onnx_models/clap-htsat-unfused_audio_with_projection.onnx".to_owned();
    //     let environment = Environment::builder()
    //         .with_execution_providers(vec![
    //             CUDA(Default::default()),
    //             CoreML(Default::default()),
    //             CPU(Default::default()),
    //         ])
    //         .with_name("CLAP")
    //         .build()?
    //         .into_arc();
    //     let audio_model = SessionBuilder::new(&environment)?
    //         .with_optimization_level(GraphOptimizationLevel::Disable)?
    //         .with_model_from_file(AUDIO_MODEL_RELATIVE_PATH)
    //         .unwrap_or_else(|_| {
    //             panic!(
    //                 "Failed to load audio embedder model from {}",
    //                 text_embedder_model_filename
    //             )
    //         });

    //     println!(Session)
    // }

    #[test]
    fn test_mel_spec_accumulation_shapes() {
        let mut mel_spec: Array2<f64> = Array2::zeros((0, 64));
        let mel_spec_chunk: ArrayBase<OwnedRepr<f64>, Dim<[usize; 2]>> = Array2::zeros((128, 64));
        mel_spec
            .append(Axis(0), mel_spec_chunk.view())
            .expect("Failed to append mel spectrogram chunk");
        println!("mel_spec shape: {:?}", mel_spec.shape());
    }
    
}
