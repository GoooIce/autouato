// An example of using `sample` to efficiently perform decent quality sample rate conversion on a
// WAV file entirely on the stack.

use dasp::{interpolate::sinc::Sinc, ring_buffer, signal, Sample, Signal};
use hound::{WavReader, WavWriter};

fn main() {
    let reader = WavReader::open("1.wav").unwrap();

    // Get the wav spec and create a target with the new desired sample rate.
    let spec = reader.spec();
    let mut target = spec;
    target.sample_rate = 10_000;

    // Read the interleaved samples and convert them to a signal.
    let samples = reader
        .into_samples()
        .filter_map(Result::ok)
        .map(i16::to_sample::<f64>);
    let signal = signal::from_interleaved_samples_iter(samples);

    // Convert the signal's sample rate using `Sinc` interpolation.
    let ring_buffer = ring_buffer::Fixed::from([[0.0]; 100]);
    let sinc = Sinc::new(ring_buffer);
    let new_signal = signal.from_hz_to_hz(sinc, spec.sample_rate as f64, target.sample_rate as f64);

    // Write the result to a new file.
    let mut writer = WavWriter::create("10k.wav", target).unwrap();
    for frame in new_signal.until_exhausted() {
        writer.write_sample(frame[0].to_sample::<i16>()).unwrap();
    }
}
