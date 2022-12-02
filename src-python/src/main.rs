// 使用tch-rs加载./silero_vad.jit模型，进行语音活动检测

use std::io::Read;

use tch::{CModule, IValue, Tensor};

fn main() {
    // 使用ffmpeg将mp4转换为pcm
    // ffmpeg -i 1.mp4 -acodec pcm_s16le -f s16le -ac 1 -ar 8000 2.pcm
    let mut pcm: Vec<f32> = Vec::new();
    let mut f = std::fs::File::open("1.pcm").unwrap();
    // np.frombuffer(out, np.int16).flatten().astype(np.float32) / 32768.0
    // pcm to f32
    let mut buf = [0u8; 2];
    while f.read(&mut buf).unwrap() == 2 {
        let v = i16::from_le_bytes(buf);
        pcm.push(v as f32 / 32768.0);
    }
    // f.read_to_end(&mut pcm).unwrap();
    let pcm = Tensor::of_slice(&pcm);

    // get audio length
    let audio_length_samples = pcm.size()[0];

    let output = get_speech_probs(pcm, audio_length_samples);

    println!("{:?}", output.len());
    let speechs = get_speech_timestamps(output, audio_length_samples);
    println!("{:?}, {:?}", speechs, speechs.len());
}

// use speech probs to get speech timestamps
fn get_speech_timestamps(speech_probs: Vec<f64>, audio_length_samples: i64) -> Vec<(i64, i64)> {
    let min_speech_samples = 16000 * 250 / 1000;
    let min_silence_samples = 16000 * 100 / 1000;
    let speech_pad_samples = 16000 * 30 / 1000;

    let mut speech_timestamps: Vec<(i64, i64)> = Vec::new();

    let threshold = 0.5;
    let neg_threshold = threshold - 0.15;
    let window_size_samples: i64 = 512;

    let mut temp_end = 0;
    let mut start = 0;
    let mut end = 0;
    let mut is_speech = false;

    for (i, speech_prob) in speech_probs.iter().enumerate() {
        if *speech_prob >= threshold && temp_end != 0 {
            temp_end = 0;
        }

        if *speech_prob >= threshold && !is_speech {
            is_speech = true;
            start = window_size_samples * i as i64;
            continue;
        }

        if *speech_prob < neg_threshold && is_speech {
            if temp_end == 0 {
                temp_end = window_size_samples * i as i64;
            }
            if (window_size_samples * i as i64) - temp_end < min_silence_samples {
                continue;
            } else {
                end = temp_end;
                if (end - start) > min_speech_samples {
                    speech_timestamps.push((start, end));
                }
                temp_end = 0;
                is_speech = false;
                continue;
            }
        }
    }

    if is_speech && (window_size_samples * speech_probs.len() as i64 - start) > min_speech_samples {
        end = window_size_samples * speech_probs.len() as i64;
        speech_timestamps.push((start, end));
    }

    // for i, speech in enumerate(speeches):
    // if i == 0:
    //     speech['start'] = int(max(0, speech['start'] - speech_pad_samples))
    // if i != len(speeches) - 1:
    //     silence_duration = speeches[i+1]['start'] - speech['end']
    //     if silence_duration < 2 * speech_pad_samples:
    //         speech['end'] += int(silence_duration // 2)
    //         speeches[i+1]['start'] = int(max(0, speeches[i+1]['start'] - silence_duration // 2))
    //     else:
    //         speech['end'] = int(min(audio_length_samples, speech['end'] + speech_pad_samples))
    //         speeches[i+1]['start'] = int(max(0, speeches[i+1]['start'] - speech_pad_samples))
    // else:
    //     speech['end'] = int(min(audio_length_samples, speech['end'] + speech_pad_samples))
    let mut clone_speech_timestamps = speech_timestamps.clone();
    for (i, speech) in clone_speech_timestamps.iter_mut().enumerate() {
        if i == 0 {
            speech.0 = std::cmp::max(0, speech.0 - speech_pad_samples);
        }
        if i != speech_timestamps.len() - 1 {
            let silence_duration = speech_timestamps[i + 1].0 - speech.1;
            if silence_duration < 2 * speech_pad_samples {
                speech.1 += silence_duration / 2;
                speech_timestamps[i + 1].0 =
                    std::cmp::max(0, speech_timestamps[i + 1].0 - silence_duration / 2);
            } else {
                speech.1 =
                    std::cmp::min(audio_length_samples as i64, speech.1 + speech_pad_samples);
                speech_timestamps[i + 1].0 =
                    std::cmp::max(0, speech_timestamps[i + 1].0 - speech_pad_samples);
            }
        } else {
            speech.1 = std::cmp::min(audio_length_samples as i64, speech.1 + speech_pad_samples);
        }
    }

    clone_speech_timestamps
}

fn get_speech_probs(audio: Tensor, audio_length_samples: i64) -> Vec<f64> {
    // 加载模型
    let mut model = CModule::load("./silero_vad.jit").unwrap();
    model.set_eval();

    // speech_probs = []
    let mut speech_probs = Vec::new();
    let window_size_samples: i64 = 512;
    for current_start_sample in
        (0..audio_length_samples).step_by(window_size_samples.try_into().unwrap())
    {
        let mut chunk = audio.slice(
            0,
            current_start_sample,
            current_start_sample + window_size_samples,
            1,
        );
        if chunk.size()[0] < window_size_samples {
            chunk = chunk.pad(&[0, window_size_samples - chunk.size()[0]], "constant", 0.0);
        }
        // chunk.print();
        let speech_prob = model
            .method_is(
                "forward",
                &[tch::IValue::Tensor(chunk), tch::IValue::Int(16000)],
            )
            .unwrap();
        // speech_prob.
        let v1 = <Tensor>::try_from(speech_prob).unwrap();

        speech_probs.push(v1.double_value(&[0]));
    }
    speech_probs
}
