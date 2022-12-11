use dasp::Signal as dasp_Signal;

// use symphonia::core::audio::AudioBuffer;
use symphonia::core::audio::AudioBufferRef;
use symphonia::core::audio::Signal;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::codecs::CODEC_TYPE_AAC;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

// use dasp::signal::MulHz;
// use dasp::Sample;
use dasp::{interpolate::sinc::Sinc, ring_buffer, signal, Sample};

pub fn read(file_name: &str) -> Result<Vec<f32>, Error> {
    // Open the media source
    let src = std::fs::File::open(file_name).expect("msg");

    let source = MediaSourceStream::new(Box::new(src), Default::default());

    // Create a probe hint using the file's extension. [Optional]
    let mut hint = Hint::new();
    hint.with_extension("mp4");

    // Use the default options when reading and decoding.
    let format_opts: FormatOptions = Default::default();
    let metadata_opts: MetadataOptions = Default::default();
    let decoder_opts: DecoderOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, source, &format_opts, &metadata_opts)
        .unwrap();
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec == CODEC_TYPE_AAC)
        .unwrap();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .expect("msg");
    let trace_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap();

    let len_packet = 0;
    let mut len_loop = 0;
    let mut ts_vec: Vec<f32> = Vec::new();

    loop {
        len_loop += 1;
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(_)) => break,
            Err(e) => panic!("msg: {:?}", e),
        };

        if packet.track_id() != trace_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(_decoder) => {
                match _decoder {
                    AudioBufferRef::F32(buf) => {
                        // into samples
                        let samples = buf
                            .chan(0)
                            .iter()
                            .map(|s| s.to_sample::<f64>())
                            .collect::<Vec<f64>>();

                        let signal = signal::from_interleaved_samples_iter(samples);

                        // 使用dasp库进行重采样
                        let ring_buffer = ring_buffer::Fixed::from([[0.0]; 100]);
                        let sinc = Sinc::new(ring_buffer);

                        let new_signal = signal.from_hz_to_hz(sinc, sample_rate.into(), 16000.0);

                        for frame in new_signal.until_exhausted() {
                            ts_vec.push(frame[0].to_sample::<f32>());
                        }
                    }
                    _ => {
                        // Repeat for the different sample formats.
                        unimplemented!()
                    }
                }
            }

            Err(e) => {
                println!("{:?}", e);
            }
        }
    }

    println!("len_packet: {}", len_packet);
    println!("len_loop: {}", len_loop);
    println!("ts_vec: {:?}", ts_vec.len());
    Ok(ts_vec)
}
