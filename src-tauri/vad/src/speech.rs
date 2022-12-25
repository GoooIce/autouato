use tch::{CModule, IValue, Tensor};

// use speech probs to get speech timestamps
// 通过语音概率获取语音时间戳
pub fn get_speech_timestamps(
    speech_probs: Vec<f64>,    // 包含语音概率的数组
    audio_length_samples: i64, // 音频长度（采样数为单位）
    sample_def: i64,           // 每个时间窗口的采样数
) -> Vec<(i64, i64)> {
    let min_speech_samples = sample_def * 250 / 1000; //语音区间的最小长度，单位为采样数。
    let min_silence_samples = sample_def * 2000 / 1000; //静音区间的最小长度，单位为采样数。
    let speech_pad_samples = sample_def * 30 / 1000; //语音区间的补偿长度，单位为采样数。

    let mut speech_timestamps: Vec<(i64, i64)> = Vec::new();

    let threshold = 0.5; //阈值，用于确认是否为语音区间
    let neg_threshold = threshold - 0.15; //阈值的相反值，用于确认是否为静音区间
    let window_size_samples: i64 = 512; //窗口大小，单位为采样数

    let mut temp_end = 0; // 临时结束时间，用于记录当前语音区间的结束时间
    let mut start = 0; // 语音区间的开始时间
    let mut _end = 0; // 语音区间的结束时间
    let mut is_speech = false; // 是否为语音区间

    // 从第一个窗口开始，遍历每个窗口
    for (i, speech_prob) in speech_probs.iter().enumerate() {
        // 如果当前窗口的语音概率大于阈值,并且之前静音区间还没有结束
        //，则将临时结束时间戳重置0
        if *speech_prob >= threshold && temp_end != 0 {
            temp_end = 0;
        }

        // 如果当前时间窗口的语音概率高于阈值
        // 则将当前时间窗口视为语音区间开始
        if *speech_prob >= threshold && !is_speech {
            is_speech = true;
            start = window_size_samples * i as i64;
            continue;
        }

        // 如果当前时间窗口的语音概率低于阈值的相反值
        // 并且之前区间是语音，则将当前时间窗口视为语音区间结束
        if *speech_prob < neg_threshold && is_speech {
            // 如果之前的静音区间还没有结束
            if temp_end == 0 {
                temp_end = window_size_samples * i as i64;
            }
            // 检查当前时间窗口是否符合静音区间的最小长度
            if (window_size_samples * i as i64) - temp_end < min_silence_samples {
                continue;
            } else {
                _end = temp_end;

                // 如果当前窗口符合静音区间的最小长度要求，则将之前的临时结束时间戳设为当前窗口的结束时间戳
                if (_end - start) > min_speech_samples {
                    speech_timestamps.push((start, _end));
                }
                // 重置临时结束时间戳和语音状态，继续遍历下一个时间窗口
                temp_end = 0;
                is_speech = false;
                continue;
            }
        }
    }

    // 如果最后一个时间窗口是语音区间，则将其视为语音区间的结束
    // 注意这里的结束时间戳应该设置为输入音频的长度
    if is_speech && (window_size_samples * speech_probs.len() as i64 - start) > min_speech_samples {
        _end = window_size_samples * speech_probs.len() as i64;
        speech_timestamps.push((start, _end));
    }

    // 根据语音区间的补偿长度，对语音区间进行补偿调整
    let mut clone_speech_timestamps = speech_timestamps.clone();
    for (i, speech) in clone_speech_timestamps.iter_mut().enumerate() {
        // 如果当前语音区间是第一个语音区间，则将语音区间的歧视时间戳向前延伸
        // ，以涵盖可能在静音区间之前被遗漏的语音信号
        if i == 0 {
            speech.0 = std::cmp::max(0, speech.0 - speech_pad_samples);
        }
        // 如果当前语音区间是最后一个语音区间，则检查语音区间和相邻的静音区间之间的间隔
        // 如果间隔小于2倍的补偿长度，则将语音区间的结束时间戳和相邻静音区间的起始时间戳评分间隔
        // 这样做的目的是让语音区间和相邻静音区间都向中间延伸，以涵盖可能在间隔处被遗漏的语音信号
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
            // 如果当前语音区间是最后一个语音区间，则将语音区间的结束时间戳向后延伸，加上补偿长度
            speech.1 = std::cmp::min(audio_length_samples as i64, speech.1 + speech_pad_samples);
        }
    }

    clone_speech_timestamps
}

pub fn get_speech_probs(audio: Tensor, audio_length_samples: i64, sample_def: i64) -> Vec<f64> {
    // tch::maybe_init_cuda();
    // println!("Cuda available: {}", tch::Cuda::is_available());
    // println!("Cudnn available: {}", tch::Cuda::cudnn_is_available());
    // let device = tch::Device::cuda_if_available();
    // 加载模型
    let mut model = CModule::load("resources/silero_vad.jit").unwrap();
    // 如果cuda可用，则将模型转移到cuda上
    // if tch::Cuda::is_available() {
    //     model.to(device, tch::Kind::Float, false);
    // }
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
            .forward_is(&[IValue::Tensor(chunk), IValue::Int(sample_def)])
            .unwrap();
        // speech_prob.
        let v1 = <Tensor>::try_from(speech_prob).unwrap();

        speech_probs.push(v1.double_value(&[0]));
    }
    speech_probs
}
