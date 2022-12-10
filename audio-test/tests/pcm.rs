use std::io::Write;

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
// use dasp::{interpolate::sinc::Sinc, ring_buffer, signal, Sample, Signal};

fn main() {
    // Open the media source
    let src = std::fs::File::open("1.mp4").expect("msg");

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

    // println!("format: {:?}", format.metadata());
    println!("track: {:?}", track);
    // 准备写入2.pcm
    // let mut pcm = std::fs::File::create("u8.pcm").expect("msg");

    let mut len_packet = 0;
    let mut len_loop = 0;
    let mut ts_vec = Vec::new();

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

        ts_vec.push(packet.ts());

        match decoder.decode(&packet) {
            Ok(_decoder) => {
                match _decoder {
                    AudioBufferRef::F32(buf) => {
                        let chan0 = buf.chan(0);

                        len_packet += chan0.len();
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
    println!("ts_vec: {:?}", ts_vec);
}
