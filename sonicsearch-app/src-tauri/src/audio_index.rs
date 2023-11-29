use futures::join;
use hound::{SampleFormat, WavReader};
use mel_spec::config::MelConfig;
use mel_spec_pipeline::{Pipeline, PipelineConfig};
use ndarray::{concatenate, Array2, Array3, Axis};
use ort::{Environment, GraphOptimizationLevel, SessionBuilder};
use rubato::{FftFixedIn, Resampler};
use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::Arc;
use std::{cmp, result};
use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
};
use tokio::try_join;

use anyhow::{Context, Result};
use futures::future::join_all;
use sqlx::SqlitePool;
use tauri::State;
use twox_hash::XxHash64;
use walkdir::WalkDir;

use crate::state::{AppState, AudioEmbedder};

fn compute_hash(file: &File) -> io::Result<String> {
    let hash_seed = 1023489u64;
    let mut hasher = XxHash64::with_seed(hash_seed);
    let mut reader = BufReader::new(file);

    let mut buffer = [0; 1024]; // Read in chunks of 1024 bytes
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.write(&buffer[..n]);
    }

    Ok(hasher.finish().to_string())
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .map(|ext| {
            let ext = ext.to_str().unwrap_or("").to_lowercase();
            ["wav"].contains(&ext.as_str())
        })
        .unwrap_or(false)
}

#[tauri::command]
pub async fn update_audio_index(app_state: State<'_, AppState>) -> result::Result<bool, String> {
    println!("Updating audio file index...");
    let mut is_indexing = app_state.is_indexing.write().await;
    if *is_indexing {
        // Should never happen, as is_indexing should be locked while index is occuring
        println!("Indexing already in progress.");
        return Err("Indexing already in progress.".into());
    } else {
        *is_indexing = true;
    }
    let user_audio_dir = dirs::audio_dir().expect("Failed to get user home directory");
    let audio_embedder = &app_state.clap_model_audio_embedder;
    let pool = app_state.pool.clone();

    let upsert_futures: Vec<_> = WalkDir::new(user_audio_dir)
        .into_iter()
        .filter(|dir| {
            dir.as_ref()
                .is_ok_and(|ok_dir| ok_dir.path().is_file() && is_audio_file(ok_dir.path()))
        })
        .map(move |dir| {
            Box::pin(upsert_audio_file(
                pool.to_owned(),
                audio_embedder,
                dir.expect("dir should exist").path().to_owned(),
            ))
        })
        .collect();
    let embedder_future = audio_embedder.process_queue();

    let (embedder_result, upsert_results) = join!(embedder_future, join_all(upsert_futures));

    embedder_result
        .context("Model should run successfully")
        .map_err(|err| format!("Failed to run audio embedder: {:?}", err))?;
    let upsert_results: (usize, Vec<String>) = upsert_results
        .into_iter()
        .map(|res| res.map_err(|err| format!("Failed to update audio index: {:?}", err)))
        .map(|res| match res {
            Ok(ok) => Ok(ok),
            Err(err) => {
                println!("{}", err);
                Err(err)
            }
        })
        .fold((0, vec![]), |mut acc, res| {
            match res {
                Ok(_) => acc.0 += 1,
                Err(err) => acc.1.push(err),
            }
            acc
        });
    println!(
        "Indexed {} files total. Success: {}, Failures: {}",
        upsert_results.0 + upsert_results.1.len(),
        upsert_results.0,
        upsert_results.1.len()
    );

    sqlx::query(
        r#"INSERT INTO vss_audio_file_segment
            SELECT embedding FROM audio_file_segment"#,
    )
    .execute(&app_state.pool)
    .await
    .map_err(|err| format!("Failed to update vss_audio_file_segment: {:?}", err))?;

    println!("\nAudio file index updated.");
    *is_indexing = false;
    Ok(true)
}

struct LoadedAudioFile {
    file_hash: String,
    file_path: String,
    file: Option<File>,
}

struct AudioFileRow {
    file_hash: String,
    file_path: String,
}

pub async fn upsert_audio_file(
    pool: SqlitePool,
    audio_embedder: &AudioEmbedder,
    path: PathBuf,
) -> Result<()> {
    println!("Upserting {} ", path.display());
    let file = File::open(&path)?;
    let audio_file = LoadedAudioFile {
        file_hash: compute_hash(&file).context("Failed to compute hash")?,
        file_path: path.to_string_lossy().into_owned(),
        file: Some(file),
    };
    let existing_row = sqlx::query_as!(
        AudioFileRow,
        r#"SELECT * FROM audio_file WHERE file_hash = ?"#,
        audio_file.file_hash
    )
    .fetch_optional(&pool)
    .await?;

    if existing_row.is_none() {
        println!("{} is new, indexing...", path.display());
        index_new_file(pool, audio_embedder, &audio_file).await?;
    } else if existing_row
        .as_ref()
        .is_some_and(|row| row.file_path != audio_file.file_path)
    {
        println!(
            "{} has moved from {}, updating path...",
            path.display(),
            existing_row.as_ref().unwrap().file_path
        );
        update_path(pool, &audio_file).await?;
    } else {
        println!("{} already indexed.", path.display());
    }

    Ok(())
}

async fn update_path(pool: SqlitePool, audio_file: &LoadedAudioFile) -> Result<()> {
    sqlx::query(r#"UPDATE audio_file SET file_path = ? WHERE file_hash = ?"#)
        .bind(&audio_file.file_path)
        .bind(&audio_file.file_hash)
        .execute(&pool)
        .await?;

    Ok(())
}

#[derive(Debug)]
struct FileSegment<'a> {
    starting_timestamp: f64,
    pcm_audio: &'a [f32],
}

#[derive(Debug)]
struct FileSegmentWithEmbedding {
    starting_timestamp: f64,
    embedding: Vec<f64>,
}

async fn index_new_file(
    pool: SqlitePool,
    audio_embedder: &AudioEmbedder,
    audio_file: &LoadedAudioFile,
) -> Result<u32> {
    // Split file into segments and compute embeddings for each segment
    // Once all are computed, insert into database

    let segments_with_embeddings = segment_and_embed_file(audio_file, audio_embedder).await?;

    // Insert all segments and audio file into database
    println!(
        "Inserting {} segments of {} into database...",
        segments_with_embeddings.len(),
        audio_file.file_path
    );
    pool.begin().await?;
    sqlx::query!(
        r#"INSERT INTO audio_file (file_hash, file_path) VALUES (?, ?)"#,
        audio_file.file_hash,
        audio_file.file_path
    )
    .execute(&pool)
    .await?;
    join_all(segments_with_embeddings.into_iter().map(|segment| (segment, pool.clone())).map(|(segment, pool)| async move {
        // TODO: consider using a raw byte array instead of JSON. Must clarify endianness.
        let encoded_embedding: String = serde_json::to_string(&(segment.embedding.to_owned().iter().map(|emb| *emb as f32).collect::<Vec<f32>>())).expect("Should be able to serialize embedding");
        sqlx::query!(
            r#"INSERT INTO audio_file_segment (file_hash, starting_timestamp, embedding) VALUES (?, ?, ?)"#,
            audio_file.file_hash,
            segment.starting_timestamp,
            encoded_embedding
        )
            .execute(&pool)
            .await.expect("Segment insertion should succeed");
    })).await;

    Ok(1)
}

async fn segment_and_embed_file(
    audio_file: &LoadedAudioFile,
    audio_embedder: &AudioEmbedder,
) -> Result<Vec<FileSegmentWithEmbedding>> {
    // Process audio file into embedded segments
    println!("Preprocessing {}...", get_file_name(&audio_file.file_path));
    let pcm_audio = preprocess_audio_file_to_pcm(audio_file)
        .await
        .context(format!(
            "Failed to preprocess audio file {}",
            audio_file.file_path
        ))?;
    println!(
        "Preprocessed {} into {} samples",
        get_file_name(&audio_file.file_path),
        pcm_audio.len()
    );
    println!(
        "Splitting {} into segments...",
        get_file_name(&audio_file.file_path)
    );
    let audio_segments = split_audio_into_segments(&pcm_audio);
    println!(
        "Split {} into {} segments with lengths {:?}",
        get_file_name(&audio_file.file_path),
        audio_segments.len(),
        audio_segments
            .iter()
            .map(|segment| segment.pcm_audio.len())
            .collect::<Vec<usize>>()
    );
    let segments_with_embeddings: Result<Vec<_>> = Result::from_iter(
        join_all(
            audio_segments
                .into_iter()
                .enumerate()
                .map(|(i, segment)| async move {
                    println!(
                        "Computing embedding for segment {} of {}...",
                        i,
                        get_file_name(&audio_file.file_path)
                    );
                    let segment_embedding =
                        compute_embedding_from_pcm(segment.pcm_audio, audio_embedder)
                            .await
                            .context("Failed to compute embedding for segment {} of {}")?;
                    Ok(FileSegmentWithEmbedding {
                        starting_timestamp: segment.starting_timestamp,
                        embedding: segment_embedding,
                    })
                }),
        )
        .await,
    );

    segments_with_embeddings
}

/// Get local path for testing, based on CARGO_MANIFEST_DIR env var
fn get_local_path(path: &str) -> Result<String> {
    let mut local_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    local_path.push(path);
    let local_path_string = local_path
        .into_os_string()
        .into_string()
        .expect("Should be able to convert path to string");
    Ok(local_path_string)
}

#[tokio::test]
async fn test_segment_and_embed_file() {
    let test_audio_file = LoadedAudioFile {
        file_hash: "fake_hash".to_string(),
        file_path: get_local_path("test_resources/audio/audio_00.wav")
            .expect("Should get local path"),
        file: None,
    };
    let test_audio_embedder = Arc::new(create_local_audio_embedder());

    let process_queue = tokio::spawn({
        let cloned_audio_embedder = test_audio_embedder.clone();
        async move { cloned_audio_embedder.to_owned().process_queue().await }
    });
    let segment_and_embed = tokio::spawn({
        let cloned_audio_embedder = test_audio_embedder.clone();
        async move { segment_and_embed_file(&test_audio_file, &cloned_audio_embedder).await }
    });

    let (process_result, embed_result) =
        try_join!(process_queue, segment_and_embed).expect("Should join futures");
    println!("{:?}", process_result);
    println!("{:?}", embed_result);
}

const TARGET_SAMPLE_RATE: u32 = 48000;
const SEGMENT_LENGTH: f32 = 10.0; // seconds
const SEGMENT_STEP: f32 = 5.0; // seconds

fn get_file_name(path: &String) -> String {
    let path = Path::new(path);
    path.file_name()
        .and_then(|file_name| file_name.to_str())
        .map(|file_name| file_name.to_string())
        .expect("Should get file name")
}

/// Process an audio file into an f32 PCM vector with a sample rate of 48kHz
async fn preprocess_audio_file_to_pcm(audio_file: &LoadedAudioFile) -> Result<Vec<f32>> {
    let file_ext = audio_file
        .file_path
        .split('.')
        .last()
        .context("Failed to get file extension")?
        .to_lowercase();
    let file_ext = file_ext.as_str();

    match file_ext {
        "wav" => {
            let wav_reader =
                // TODO: this probably redundantly opens the file, which can take a while.
                // Easy perf win: use audio_file.file instead.
                WavReader::open(&audio_file.file_path).context("Failed to read .wav file")?;
            let wav_spec = wav_reader.spec();
            let initial_seconds = wav_reader.duration() as f32 / wav_spec.sample_rate as f32;
            println!(
                "Before preprocessing, {} has a sample rate of {} and a length of {} samples, for a duration of {} seconds",
                get_file_name(&audio_file.file_path),
                wav_spec.sample_rate,
                wav_reader.duration(),
                initial_seconds
            );
            let mut wav_samples: Vec<f32> = match wav_spec.sample_format {
                SampleFormat::Float => wav_reader
                    .into_samples::<f32>()
                    .map(|sample| sample.expect("Failed to read .wav sample"))
                    .collect(),
                SampleFormat::Int => wav_reader
                    .into_samples::<i32>()
                    .map(|sample| {
                        let sample = sample.expect("Failed to read .wav sample");
                        // Normalize to f32
                        (sample as f32 / i32::MAX as f32) * f32::MAX
                    })
                    .collect(),
            };

            if wav_spec.channels != 1 {
                // Sum to mono
                wav_samples = wav_samples
                    .chunks(wav_spec.channels as usize)
                    .map(|chunk| chunk.iter().sum::<f32>() / wav_spec.channels as f32)
                    .collect();
            }
            if wav_spec.sample_rate != TARGET_SAMPLE_RATE {
                wav_samples = resample(wav_samples.as_ref(), wav_spec.sample_rate)?;
            }
            let final_seconds = wav_samples.len() as f32 / TARGET_SAMPLE_RATE as f32;
            println!(
                "Resampled {} to {} samples, for a duration of {} seconds",
                get_file_name(&audio_file.file_path),
                wav_samples.len(),
                final_seconds
            );
            if (final_seconds - initial_seconds).abs() > 0.1 {
                return Err(anyhow::anyhow!(
                    "Resampled audio file {} has a duration of {} seconds, but should have a duration of {} seconds",
                    get_file_name(&audio_file.file_path),
                    final_seconds,
                    initial_seconds
                ));
            }
            Ok(wav_samples)
        }
        _ => Err(anyhow::anyhow!(
            "Unsupported file extension: {} for file {}",
            file_ext,
            audio_file.file_path
        )),
    }
}

fn resample(samples: &[f32], source_sample_rate: u32) -> Result<Vec<f32>> {
    let initial_seconds = samples.len() as f32 / source_sample_rate as f32;
    let mut resampler = FftFixedIn::<f32>::new(
        source_sample_rate
            .try_into()
            .expect("source_sample_rate should be converted"),
        TARGET_SAMPLE_RATE
            .try_into()
            .expect("TARGET_SAMPLE_RATE should be converted"),
        samples.len(),
        1,
        1,
    )
    .context("Failed to create resampler")?;

    let resampled_samples = resampler
        .process(&[samples], None)
        .context("Failed to resample")
        .map(|res| {
            res.get(0)
                .expect("Resampled audio should have 1 channel")
                .to_vec()
        })?;
    let resampled_seconds = resampled_samples.len() as f32 / TARGET_SAMPLE_RATE as f32;
    if (resampled_seconds - initial_seconds).abs() > 0.1 {
        return Err(anyhow::anyhow!(
            "During resampling, audio file with duration of {} seconds was resampled to {} seconds",
            initial_seconds,
            resampled_seconds
        ));
    }
    Ok(resampled_samples)
}

fn split_audio_into_segments(pcm_audio: &[f32]) -> Vec<FileSegment> {
    let segment_length_samples = (TARGET_SAMPLE_RATE as f32 * SEGMENT_LENGTH) as usize;
    let segment_step_samples = (TARGET_SAMPLE_RATE as f32 * SEGMENT_STEP) as usize;

    let mut segments = vec![];
    let mut current_sample = 0;
    for _ in 0..pcm_audio.len() / segment_step_samples {
        let final_sample = cmp::min(current_sample + segment_length_samples, pcm_audio.len() - 1);
        let segment = FileSegment {
            starting_timestamp: current_sample as f64 / TARGET_SAMPLE_RATE as f64,
            pcm_audio: &pcm_audio[current_sample..final_sample],
        };
        segments.push(segment);
        current_sample += segment_step_samples;
    }

    let segments = match segments.len() {
        0 => vec![FileSegment {
            starting_timestamp: 0.0,
            pcm_audio,
        }],
        _ => segments,
    };
    println!("Split into {} segments", segments.len());
    segments
}

struct MelConfigSettings {
    fft_size: usize,
    sample_rate: f64,
    hop_size: usize,
    n_mels: usize,
    _power: f64,
}

/// Config based on `clap_export.ipynb` experiments
const MEL_CONFIG: MelConfigSettings = MelConfigSettings {
    fft_size: 1024,
    sample_rate: TARGET_SAMPLE_RATE as f64,
    hop_size: 480,
    n_mels: 64,
    _power: 2.0,
};

const TARGET_LENGTH: usize = 1001;
fn reshape_mel_spec(mel_spec: Array2<f64>) -> Result<Array3<f64>> {
    println!("Reshaping mel_spec of shape {:?}", mel_spec.shape());
    let transposed_mel_spec = mel_spec.t().to_owned();
    if transposed_mel_spec.len_of(Axis(0)) == TARGET_LENGTH {
        return Ok(transposed_mel_spec.insert_axis(Axis(0)));
    } else if transposed_mel_spec.len_of(Axis(0)) == 0 {
        return Err(anyhow::anyhow!("Mel spectrogram is empty"));
    }

    let mut result: Array2<f64> = transposed_mel_spec.clone();
    while result.len_of(Axis(0)) < TARGET_LENGTH {
        let result_len = result.len_of(Axis(0));
        let padding_left = TARGET_LENGTH - result_len;
        let slice_bound = match padding_left > result_len {
            true => result_len,
            false => padding_left,
        };

        let view_to_add = transposed_mel_spec.slice_axis(
            Axis(0),
            ndarray::Slice {
                start: (0),
                end: (Some(
                    slice_bound
                        .try_into()
                        .expect("slice_bound should always be positive"),
                )),
                step: (1),
            },
        );
        println!(
            "Adding view of shape {:?} to result of shape {:?}",
            view_to_add.shape(),
            result.shape()
        );
        result = concatenate![Axis(0), result, view_to_add];
    }

    Ok(result.insert_axis(Axis(0)))
}

fn compute_mel_spec_from_pcm(segment_pcm: &[f32]) -> Result<Array3<f64>> {
    println!(
        "Computing mel spectrogram for pcm of length {}",
        segment_pcm.len()
    );
    // Compute mel spectrogram
    let mel_config = MelConfig::new(
        MEL_CONFIG.fft_size,
        MEL_CONFIG.hop_size,
        MEL_CONFIG.n_mels,
        MEL_CONFIG.sample_rate,
    );
    // TODO: make sure this doesn't have weird Voice Activity Detection side effects
    let pipeline_config = PipelineConfig::new(mel_config, None);
    let mut pipeline = Pipeline::new(pipeline_config);

    let pipeline_join_handles = pipeline.start();
    let rx_clone = pipeline.rx();
    pipeline.send_pcm(segment_pcm)?;
    pipeline.close_ingress();

    let mut mel_spec: Array2<f64> = Array2::zeros((MEL_CONFIG.n_mels, 0));
    loop {
        match rx_clone.recv() {
            Ok((mel_idx, mel_spec_chunk)) => {
                print!("\rReceived mel spectrogram chunk {:?}", mel_idx);
                mel_spec
                    .append(Axis(1), mel_spec_chunk.view())
                    .context(format!(
                "Failed to append mel spectrogram chunk of shape {:?} to mel_spec of shape {:?}",
                mel_spec_chunk.shape(),
                mel_spec.shape()
            ))?;
            }
            Err(err) => {
                println!("Failed to receive mel spectrogram chunk: {:?}", err);
                break;
            }
        }
    }

    for handle in pipeline_join_handles {
        handle.join().expect("Pipeline should join");
    }

    let reshaped_mel_spec = reshape_mel_spec(mel_spec).context("Failed to reshape mel spec")?;
    Ok(reshaped_mel_spec)
}

#[test]
fn test_int_to_float_cast() {
    println!("{} -> {}", 15 as i32, (15 as i32) as f32);
}
#[test]
fn test_compute_mel_spec_from_pcm_with_zeros() {
    // 10 seconds of 48kHz silence
    let test_segment_pcm = vec![0.0; 48000 * 10];
    let result = compute_mel_spec_from_pcm(&test_segment_pcm);
    println!("{:?}", result);
}

#[test]
fn test_compute_mel_spec_from_pcm_with_no_length() {
    // 0 seconds of 48kHz silence
    let test_segment_pcm = vec![0.0; 0];
    let result = compute_mel_spec_from_pcm(&test_segment_pcm);
    println!("{:?}", result);
}

async fn compute_embedding_from_pcm(
    segment_pcm: &[f32],
    audio_embedder: &AudioEmbedder,
) -> Result<Vec<f64>> {
    let mel_spec = compute_mel_spec_from_pcm(segment_pcm)?;
    // Compute embedding
    let embedding = compute_embedding_from_mel_spec(mel_spec, audio_embedder).await?;

    Ok(embedding)
}

fn create_local_audio_embedder() -> AudioEmbedder {
    let audio_embedder_model_path =
        get_local_path("onnx_models/clap-htsat-unfused_audio_with_projection.onnx")
            .expect("Should get local path");
    let environment = Environment::builder()
        .with_name("CLAP")
        .build()
        .expect("Failed to create environment")
        .into_arc();
    let audio_embedder_session = SessionBuilder::new(&environment)
        .expect("Failed to create session builder")
        .with_optimization_level(GraphOptimizationLevel::Disable)
        .expect("Failed to set optimization level")
        .with_model_from_file(audio_embedder_model_path.clone())
        .unwrap_or_else(|_| {
            panic!(
                "Failed to load audio embedder model from {}",
                audio_embedder_model_path
            )
        });
    AudioEmbedder::new(audio_embedder_session)
}

// #[tokio::test]
// async fn test_compute_embedding_from_pcm() {
//     // 10 seconds of 48kHz silence
//     let test_segment_pcm = vec![0.0; 48000 * 10];
//     let test_audio_embedder = create_local_audio_embedder();
//     let process_queue_handle = tokio::spawn(test_audio_embedder.process_queue());
//     let result = compute_embedding_from_pcm(&test_segment_pcm, &test_audio_embedder)
//         .await
//         .unwrap();

//     tokio::join!(process_queue_handle);
//     println!("{:?}", result);
// }

async fn compute_embedding_from_mel_spec(
    mel_spec: Array3<f64>,
    audio_embedder: &AudioEmbedder,
) -> Result<Vec<f64>> {
    println!(
        "Computing embedding for mel_spec of shape {:?}",
        mel_spec.shape()
    );
    let embedding = audio_embedder
        .queue_for_batch_processing(mel_spec.to_owned())
        .await?;

    Ok(embedding.to_vec())
}

pub fn get_search_results(search_string: &str, _pool: &SqlitePool) -> Result<Vec<String>> {
    // Stubbed search algorithm: pick 10 random audio files

    Ok(vec![format!("~/{}", search_string.to_owned()).to_string()])
}
