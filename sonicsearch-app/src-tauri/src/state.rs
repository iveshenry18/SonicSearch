use std::sync::Arc;

use anyhow::Result;
use futures::{
    channel::oneshot::{self, Sender},
    lock::Mutex,
};

use ndarray::{stack, Array1, Array3, Axis, CowArray};
use ort::Session;
use sqlx::SqlitePool;
use tauri::async_runtime::RwLock;
use tokio::sync::Notify;

pub struct AppState {
    pub pool: SqlitePool,
    pub clap_model_text_embedder: Arc<Mutex<Session>>,
    pub clap_model_audio_embedder: AudioEmbedder,
    pub is_indexing: RwLock<bool>,
}

pub struct AudioEmbedder {
    session: Arc<Mutex<Session>>,
    input_queue: Arc<Mutex<Vec<(Array3<f64>, Sender<Array1<f64>>)>>>,
    queue_notify: Arc<Notify>,
}

/// This is a wrapper around the ONNX runtime session that allows us to queue up
/// audio for batch processing. Multiple threads can add inputs to the queue,
/// and a single thread will process the queue in batches.
impl AudioEmbedder {
    pub fn new(session: Session) -> Self {
        Self {
            session: Arc::new(Mutex::new(session)),
            input_queue: Arc::new(Mutex::new(Vec::new())),
            queue_notify: Arc::new(Notify::new()),
        }
    }

    /// Other threads can call this to queue up audio for batch processing.
    /// This is a blocking call that will wait until the input has been processed,
    /// then return the output for the given input.
    pub async fn queue_for_batch_processing(&self, input: Array3<f64>) -> Array1<f64> {
        let (sender, receiver) = oneshot::channel();

        {
            let mut input_queue = self.input_queue.lock().await;
            (*input_queue).push((input, sender));
        }
        // If process_queue is waiting for inputs, wake it up
        self.queue_notify.notify_one();

        receiver.await.unwrap()
    }

    /// This is the function that actually processes the queue.
    /// It continually runs and waits for inputs to be added to the queue.
    pub async fn process_queue(&self) -> Result<()> {
        loop {
            // Wait until there are inputs to process
            self.queue_notify.notified().await;

            let mut inputs_to_process = Vec::new();
            let session = self.session.lock().await;
            // Read from the queue and release the lock
            {
                let mut input_queue = self.input_queue.lock().await;
                inputs_to_process.append(input_queue.as_mut());
            }

            println!("Processing {} inputs", inputs_to_process.len());
            let (input_batch, senders): (Vec<Array3<f64>>, Vec<Sender<Array1<f64>>>) =
                inputs_to_process
                    .into_iter()
                    .map(|(input, sender)| (input, sender))
                    .unzip();
            // Process the inputs in a batch
            let input_batch = stack(
                Axis(0),
                input_batch
                    .iter()
                    .map(|x| x.view())
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .unwrap();
            let outputs = session
                .run(vec![ort::Value::from_array(
                    session.allocator(),
                    &CowArray::from(input_batch.into_dyn()),
                )
                .unwrap()])
                .unwrap();

            // Send the outputs back to the threads that requested them
            // This is probably wrong and maybe also takes fourteen years
            let outputs: Vec<Array1<f64>> = outputs
                .into_iter()
                .map(|output| {
                    let output = output.try_extract().unwrap();
                    let output = output.view();
                    let output = output.as_slice().unwrap();
                    Array1::from_shape_vec(output.len(), output.to_vec()).unwrap()
                })
                .collect();

            for (output, sender) in outputs.into_iter().zip(senders.into_iter()) {
                sender.send(output).unwrap();
            }
        }
    }
}
