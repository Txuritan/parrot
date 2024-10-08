use std::process::{Child, Command, Stdio};

use songbird::input::{
    error::{Error, Result},
    Codec, Container, Input, Metadata, Reader,
};

pub async fn ffmpeg(mut source: Child, metadata: Metadata, pre_args: &[&str]) -> Result<Input> {
    let ffmpeg_args = [
        "-i",
        "-", // read from stdout
        "-f",
        "s16le", // use PCM signed 16-bit little-endian format
        "-ac",
        "2", // set two audio channels
        "-ar",
        "48000", // set audio sample rate of 48000Hz
        "-acodec",
        "pcm_f32le",
        "-",
    ];

    let taken_stdout = source.stdout.take().ok_or(Error::Stdout)?;

    let ffmpeg = Command::new("ffmpeg")
        .args(pre_args)
        .args(ffmpeg_args)
        .stdin(taken_stdout)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;

    let reader = Reader::from(vec![source, ffmpeg]);

    let input = Input::new(
        true,
        reader,
        Codec::FloatPcm,
        Container::Raw,
        Some(metadata),
    );

    Ok(input)
}
