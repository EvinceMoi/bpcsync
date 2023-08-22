mod bpc;

use std::sync::mpsc::channel;

use bpc::BPCWave;
use rodio::{OutputStream, source::Source, Sink};
use anyhow::{Result, Context};
use ctrlc;



fn main() -> Result<()> {
    let (tx, rx) = channel();
    ctrlc::set_handler(move || _ = tx.send(()))?;

    let (_stream, stream_handle) = OutputStream::try_default()
        .with_context(|| format!("unable to open default output device"))?;
    let sink = Sink::try_new(&stream_handle)
        .with_context(|| format!("failed to create sink"))?;

    let source = BPCWave::new();
    sink.append(source.amplify(2.));

    sink.play();
    rx.recv()?;
    sink.clear();

    Ok(())
}

