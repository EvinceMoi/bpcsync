mod bpc;


use std::sync::mpsc::channel;



use anyhow::{Context, Result};
use bpc::BPCWave;
use ctrlc;
use rodio::{OutputStream, Sink};

fn main() -> Result<()> {
    let (tx, rx) = channel();
    ctrlc::set_handler(move || _ = tx.send(()))?;

    let (_stream, stream_handle) = OutputStream::try_default()
        .with_context(|| format!("unable to open default output device"))?;
    let sink = Sink::try_new(&stream_handle).with_context(|| format!("failed to create sink"))?;

    let source = BPCWave::new();
    sink.append(source);

    sink.play();
    rx.recv()?;
    sink.stop();

    Ok(())
}
