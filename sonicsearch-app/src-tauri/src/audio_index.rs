use futures::join;
use hound::{SampleFormat, WavReader};
use mel_spec::config::MelConfig;
use mel_spec_pipeline::{Pipeline, PipelineConfig};
use ndarray::{concatenate, Array2, Array3, Axis};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::hash::Hasher;
use std::path::PathBuf;
use std::{cmp, result};
use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
};

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

struct FileSegment<'a> {
    starting_timestamp: f64,
    pcm_audio: &'a [f32],
}

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

    // Process audio file into embedded segments
    println!("Preprocessing {}...", audio_file.file_path);
    let pcm_audio = preprocess_audio_file_to_pcm(audio_file)
        .await
        .context(format!(
            "Failed to preprocess audio file {}",
            audio_file.file_path
        ))?;
    println!("Splitting {} into segments...", audio_file.file_path);
    let audio_segments = split_audio_into_segments(&pcm_audio);
    println!(
        "Computing embeddings for {} segments of {}...",
        audio_segments.len(),
        audio_file.file_path
    );
    let segments_with_embeddings = join_all(audio_segments.into_iter().enumerate().map(
        |(i, segment)| async move {
            println!(
                "Computing embedding for segment {} of {}...",
                i, audio_file.file_path
            );
            let segment_embedding = compute_embedding_from_pcm(segment.pcm_audio, audio_embedder)
                .await
                .expect("Failed to compute embedding");
            FileSegmentWithEmbedding {
                starting_timestamp: segment.starting_timestamp,
                embedding: segment_embedding,
            }
        },
    ))
    .await;

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

const TARGET_SAMPLE_RATE: u32 = 48000;
const SEGMENT_LENGTH: f32 = 10.0; // seconds
const SEGMENT_STEP: f32 = 5.0; // seconds

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
                WavReader::open(&audio_file.file_path).context("Failed to read .wav file")?;
            let wav_spec = wav_reader.spec();
            let mut wav_samples: Vec<f32> = match wav_spec.sample_format {
                SampleFormat::Float => wav_reader
                    .into_samples::<f32>()
                    .map(|sample| sample.expect("Failed to read .wav sample"))
                    .collect(),
                SampleFormat::Int => wav_reader
                    .into_samples::<i32>()
                    .map(|sample| {
                        let sample = sample.expect("Failed to read .wav sample");
                        sample as f32 / i32::MAX as f32
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
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let mut resampler = SincFixedIn::<f32>::new(
        TARGET_SAMPLE_RATE as f64 / source_sample_rate as f64,
        2.0,
        params,
        1024,
        1,
    )
    .context("Failed to create resampler")?;

    resampler
        .process(&[samples], None)
        .context("Failed to resample")
        .map(|res| {
            res.get(0)
                .expect("Resampled audio should have 1 channel")
                .to_vec()
        })
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
fn reshape_mel_spec(mel_spec: Array2<f64>) -> Array3<f64> {
    let mut result: Array2<f64> = mel_spec.clone();
    while result.len_of(Axis(1)) < TARGET_LENGTH {
        let result_len = result.len_of(Axis(1));
        let padding_left = TARGET_LENGTH - result_len;
        let slice_bound = match padding_left > result_len {
            true => result_len,
            false => padding_left,
        };

        let view_to_add = mel_spec.slice_axis(
            Axis(1),
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
        result = concatenate![Axis(1), result, view_to_add];
    }

    result.insert_axis(Axis(0))
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

    let rx_clone = pipeline.rx();
    let pipeline_join_handles = pipeline.start();
    pipeline.send_pcm(segment_pcm)?;
    pipeline.close_ingress();

    let mel_spec = rx_clone.recv().expect("mel_spec should have run").1;

    // TODO: this deadlocks
    for handle in pipeline_join_handles {
        handle.join().expect("Pipeline should join");
    }

    // Repeat-pad to 1001 frames
    Ok(reshape_mel_spec(mel_spec))
}

async fn compute_embedding_from_pcm(
    segment_pcm: &[f32],
    audio_embedder: &AudioEmbedder,
) -> Result<Vec<f64>> {
    let mel_spec = compute_mel_spec_from_pcm(segment_pcm)?;
    // Compute embedding
    let embedding = compute_embedding_from_mel_spec(mel_spec, audio_embedder).await;

    Ok(embedding)
}

async fn compute_embedding_from_mel_spec(
    mel_spec: Array3<f64>,
    audio_embedder: &AudioEmbedder,
) -> Vec<f64> {
    println!(
        "Computing embedding for mel_spec of shape {:?}",
        mel_spec.shape()
    );
    let embedding = audio_embedder
        .queue_for_batch_processing(mel_spec.to_owned())
        .await;

    embedding.to_vec()
}

pub fn get_search_results(search_string: &str, _pool: &SqlitePool) -> Result<Vec<String>> {
    // Stubbed search algorithm: pick 10 random audio files

    Ok(vec![format!("~/{}", search_string.to_owned()).to_string()])
}
