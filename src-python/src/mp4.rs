use mp4::Result;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<()> {
    let f = File::open("1.mp4").unwrap();
    let size = f.metadata()?.len();
    let reader = BufReader::new(f);

    let mut mp4 = mp4::Mp4Reader::read_header(reader, size)?;

    // Print boxes.
    println!("major brand: {}", mp4.ftyp.major_brand);
    println!("timescale: {}", mp4.moov.mvhd.timescale);

    // Use available methods.
    println!("size: {}", mp4.size());

    let mut compatible_brands = String::new();
    for brand in mp4.compatible_brands().iter() {
        compatible_brands.push_str(&brand.to_string());
        compatible_brands.push_str(",");
    }
    println!("compatible brands: {}", compatible_brands);
    println!("duration: {:?}", mp4.duration());

    // Track info.
    for track in mp4.tracks().values() {
        println!(
            "track: #{}({}) {} : {}",
            track.track_id(),
            track.language(),
            track.track_type()?,
            track.box_type()?,
        );
    }

    // for track_id in mp4.tracks().keys().copied().collect::<Vec<u32>>() {
    let sample_count = mp4.sample_count(2).unwrap();

    for sample_idx in 0..sample_count {
        let sample_id = sample_idx + 1;
        let sample = mp4.read_sample(track_id, sample_id);

        if let Some(ref samp) = sample.unwrap() {
            // println!(
            //     "[{}] start_time={} duration={} rendering_offset={} size={} is_sync={}",
            //     sample_id,
            //     samp.start_time,
            //     samp.duration,
            //     samp.rendering_offset,
            //     samp.bytes.len(),
            //     samp.is_sync,
            // );
            // 转换为pcm
            let mut decoder = mp4::audio::AudioDecoder::new(&mp4, track_id).unwrap();
            let mut buffer = mp4::audio::AudioBuffer::new();
            let mut pcm = Vec::new();
            decoder.decode(&samp.bytes, &mut buffer).unwrap();
            pcm.extend_from_slice(buffer.samples());
            // println!("pcm: {:?}", pcm);
        }
        // }
    }

    Ok(())
}
