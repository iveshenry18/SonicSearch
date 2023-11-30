use anyhow::{Context, Result};
use futures::lock::Mutex;
use ndarray::{stack, Array1, Array3, Axis, CowArray};
use ort::Session;
use std::sync::Arc;
use tokio::sync::{
    oneshot::{self, Sender},
    Notify,
};

pub struct MelSpecAndSender(Array3<f64>, Sender<Array1<f32>>);

pub struct AudioEmbedder {
    pub(crate) session: Arc<Mutex<Session>>,
    pub(crate) input_queue: Arc<Mutex<Vec<MelSpecAndSender>>>,
    pub(crate) queue_has_contents: Arc<Notify>,
    pub(crate) stop_processing_queue: Arc<Notify>,
}

/// This is a wrapper around the ONNX runtime session that allows us to queue up
/// audio for batch processing. Multiple threads can add inputs to the queue,
/// and a single thread will process the queue in batches.
impl AudioEmbedder {
    pub fn new(session: Session) -> Self {
        Self {
            session: Arc::new(Mutex::new(session)),
            input_queue: Arc::new(Mutex::new(Vec::new())),
            queue_has_contents: Arc::new(Notify::new()),
            stop_processing_queue: Arc::new(Notify::new()),
        }
    }

    /// Other threads can call this to queue up audio for batch processing.
    /// This is a blocking call that will wait until the input has been processed,
    /// then return the output for the given input.
    pub async fn queue_for_batch_processing(&self, input: Array3<f64>) -> Result<Array1<f32>> {
        let (sender, receiver) = oneshot::channel();

        {
            let mut input_queue = self.input_queue.lock().await;
            (*input_queue).push(MelSpecAndSender(input, sender));
        }
        // If process_queue is waiting for inputs, wake it up
        self.queue_has_contents.notify_one();

        let result = receiver
            .await
            .context("Did not receive output from audio embedder")?;

        println!("Received output of shape {:?}", result.shape());
        Ok(result)
    }

    /// This is the function that actually processes the queue.
    /// It continually runs and waits for inputs to be added to the queue.
    pub async fn begin_processing_queue(&self) -> Result<()> {
        println!("Starting to process queue");
        loop {
            let mut inputs_to_process = Vec::new();
            let session = self.session.lock().await;
            // Read from the queue and release the lock
            {
                let mut input_queue = self.input_queue.lock().await;
                inputs_to_process.append(input_queue.as_mut());
            }
            if inputs_to_process.is_empty() {
                print!("No inputs to process. ");
                // block until either queue_has_contents or stop_processing_queue notifies
                // if queue_has_contents is notified, then we continue processing
                // if stop_processing_queue is notified, then we break
                tokio::select! {
                    _ = self.queue_has_contents.notified() => {
                        // Continue processing
                        continue
                    }
                    _ = self.stop_processing_queue.notified() => {
                        // Break
                        break;
                    }
                }
            }

            println!("Embedding {} input(s)", inputs_to_process.len());
            let (input_batch, senders): (Vec<Array3<f64>>, Vec<Sender<Array1<f32>>>) =
                inputs_to_process
                    .into_iter()
                    .map(|MelSpecAndSender(input, sender)| (input, sender))
                    .unzip();
            // Process the inputs in a batch
            let input_batch = stack(
                Axis(0),
                input_batch
                    .iter()
                    .map(|x| x.view())
                    .collect::<Vec<_>>()
                    .as_slice(),
            )?;

            let outputs = session
                .run(vec![ort::Value::from_array(
                    session.allocator(),
                    &CowArray::from(input_batch.mapv(|x| x as f32).into_dyn()),
                )
                .context("Failed to create ort::Value from array")?])
                .context("Failed to run session")?;

            // Send the outputs back to the threads that requested them
            // This is definitely wrong but actually is reasonably fast
            let outputs: Vec<Array1<f32>> = outputs
                .get(0)
                .context("Output 0 should contain embeddings")?
                .try_extract::<f32>()
                .context("Failed to extract embeddings")?
                .view()
                .axis_iter(Axis(0))
                .map(|x| {
                    Ok(x.to_shape((x.len(),))
                        .context("Failed to reshape output")?
                        .to_owned())
                })
                .collect::<Result<Vec<_>>>()?;
            assert_eq!(outputs.len(), senders.len());
            println!(
                "Finished embedding. Sending {} outputs of size {}.",
                outputs.len(),
                outputs
                    .get(0)
                    .context("Failed to have at least one output")?
                    .len()
            );

            for (output, sender) in outputs.into_iter().zip(senders.into_iter()) {
                println!("Sending output of shape {:?} to sender", output.shape());
                sender
                    .send(output)
                    .expect("Failed to send output");
                println!("Sent output");
            }
        }
        println!("Exiting process queue");
        Ok(())
    }

    pub fn stop_processing_queue(&self) {
        self.stop_processing_queue.notify_one();
    }
}
