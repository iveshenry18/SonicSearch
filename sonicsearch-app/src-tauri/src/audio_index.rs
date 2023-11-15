use hound::WavReader;
use mel_spec::prelude::*;
use ort::Session;
use ort::tensor::OrtOwnedTensor;
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

use crate::state::AppState;

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
    // Implement logic to determine if a file is an audio file
    // This could be by extension or by inspecting the file content
    path.extension()
        .map(|ext| {
            let ext = ext.to_str().unwrap_or("").to_lowercase();
            // Define your supported audio file extensions here
            ["mp3", "wav", "flac", "m4a", "aiff"].contains(&ext.as_str())
        })
        .unwrap_or(false)
}

#[tauri::command]
pub async fn update_audio_index(app_state: State<'_, AppState>) -> result::Result<bool, usize> {
    println!("Updating audio file index...");
    // TODO: change this such that the frontend can Read (but not modify) the is_indexing state
    let mut is_indexing = app_state.is_indexing.lock().await;
    if *is_indexing {
        // Should never happen, as is_indexing should be locked while index is occuring
        println!("Indexing already in progress.");
        return Ok(false);
    } else {
        *is_indexing = true;
    }
    let user_audio_dir = dirs::audio_dir().expect("Failed to get user home directory");
    let audio_model = app_state
        .clap_model_audio_embedder
        .lock()
        .await;

    let upsert_futures: Vec<_> = WalkDir::new(user_audio_dir)
        .into_iter()
        .filter(|dir| {
            dir.as_ref()
                .is_ok_and(|ok_dir| ok_dir.path().is_file() && is_audio_file(ok_dir.path()))
        })
        .map(|dir| (dir, app_state.pool.clone()))
        .map(|(dir, pool)| {
            Box::pin(upsert_audio_file(
                pool.to_owned(),
                audio_model.to_owned(),
                dir.expect("dir should exist").path().to_owned(),
            ))
        })
        .collect();

    let has_any_error = join_all(upsert_futures)
        .await
        .into_iter()
        .any(|res| res.is_err());

    println!("\nAudio file index updated.");
    *is_indexing = false;
    Ok(!has_any_error)
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

pub async fn upsert_audio_file(pool: SqlitePool, audio_model: Session, path: PathBuf) -> Result<()> {
    println!("Loading {} ", path.display());
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
        println!("TODO: index_new_file for {}", path.display());
        index_new_file(pool, audio_model, &audio_file).await?;
    } else if existing_row.is_some_and(|row| row.file_path != audio_file.file_path) {
        println!("TODO: update path for {}", path.display());
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

struct FileSegmentWithEmbedding<'a> {
    starting_timestamp: f64,
    pcm_audio: &'a [f32],
    embedding: Vec<u8>,
}

async fn index_new_file(pool: SqlitePool, audio_model: Session, audio_file: &LoadedAudioFile) -> Result<()> {
    // Split file into segments and compute embeddings for each segment
    // Once all are computed, insert into database

    // Process audio file into embedded segments
    let pcm_audio = preprocess_audio_file_to_pcm(audio_file).await?;
    let audio_segments = split_audio_into_segments(&pcm_audio);
    let segments_with_embeddings = join_all(audio_segments.into_iter().map(
        |segment: FileSegment| async move {
            let segment_embedding = compute_embedding(segment.pcm_audio, audio_model)
                .await
                .expect("Failed to compute embedding");
            FileSegmentWithEmbedding {
                starting_timestamp: segment.starting_timestamp,
                pcm_audio: segment.pcm_audio,
                embedding: segment_embedding,
            }
        },
    ))
    .await;

    // Insert all segments and audio file into database
    pool.begin().await?;
    sqlx::query!(
        r#"INSERT INTO audio_file (file_hash, file_path) VALUES (?, ?)"#,
        audio_file.file_hash,
        audio_file.file_path
    )
    .execute(&pool)
    .await?;
    join_all(segments_with_embeddings.into_iter().map(|segment| (segment, pool.clone())).map(|(segment, pool)| async move {
        sqlx::query!(
            r#"INSERT INTO audio_file_segment (file_hash, starting_timestamp, embedding) VALUES (?, ?, ?)"#,
            audio_file.file_hash,
            segment.starting_timestamp,
            segment.embedding
        )
            .execute(&pool)
            .await.expect("Segment insertion should succeed");
    })).await;

    Ok(())
}
const TARGET_SAMPLE_RATE: u32 = 48000;
const SEGMENT_LENGTH: f32 = 10.0; // seconds
const SEGMENT_STEP: f32 = 5.0; // seconds

/// Process an audio file into an f32 PCM vector with a sample rate of 48kHz
async fn preprocess_audio_file_to_pcm(audio_file: &LoadedAudioFile) -> Result<Vec<f32>> {
    // TODO: implement
    let file_ext = audio_file
        .file_path
        .split('.')
        .last()
        .context("Failed to get file extension")?
        .to_lowercase();
    let file_ext = file_ext.as_str();

    match file_ext {
        "wav" => {
            let wav_file = audio_file.file.as_ref().context(".wav file should exist")?;
            let wav_reader = WavReader::new(wav_file).context("Failed to read .wav file")?;
            let wav_spec = wav_reader.spec();
            // Feeling ~60% confident that this will handle various bit depths correctly
            let mut wav_samples: Vec<f32> = wav_reader
                .into_samples::<f32>()
                .map(|sample| sample.expect("Failed to read .wav sample"))
                .collect();

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
        _ => Err(anyhow::anyhow!("Unsupported file extension: {}", file_ext)),
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

    println!("Split into {} segments", segments.len());
    segments
}

struct MelConfig {
    fft_size: usize,
    sample_rate: usize,
    hop_size: usize,
    n_mels: usize,
    power: f64,
}

/// Config based on `clap_export.ipynb` experiments
const MEL_CONFIG: MelConfig = 
    MelConfig {
        fft_size: 1024,
        sample_rate: TARGET_SAMPLE_RATE as usize,
        hop_size: 480,
        n_mels: 64,
        power: 2.0,
};



async fn compute_embedding(segment_pcm: &[f32], audio_model: Session) -> result::Result<Vec<u8>, ()> {
    // TODO: implement embedding computation
    
    // Compute mel spectrogram
    let mel_spec = MelSpec::new(
        MEL_CONFIG.fft_size,
        MEL_CONFIG.sample_rate,
        MEL_CONFIG.hop_size,
        MEL_CONFIG.n_mels,
        MEL_CONFIG.power,
    );
    let mel_spec = mel_spec.compute(segment_pcm).expect("Failed to compute mel spectrogram");

    // Compute embedding
    let embedding = compute_embedding_from_mel_spec(mel_spec, audio_model).await;

    Ok(vec![])
}

async fn compute_embedding_from_mel_spec(mel_spec: MelSpec, audio_model: Session) -> Vec<f32> {
    let outputs = audio_model.run(vec![mel_spec.data]).expect("Failed to run audio model");
    // Get outputs[0] (outputs[1] is last_hidden state)
    let embedding: OrtOwnedTensor<f32, _> = outputs[0].try_extract().expect("Failed to extract embedding");
    embedding.view().slice(s![0, ..]).to_vec()
}

pub fn get_search_results(search_string: &str, _pool: &SqlitePool) -> Result<Vec<String>> {
    // Stubbed search algorithm: pick 10 random audio files

    Ok(vec![format!("~/{}", search_string.to_owned()).to_string()])
}
