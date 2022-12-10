import torch
import torchaudio
from typing import List
import torch.nn.functional as F
import warnings
import whisper
import time

languages = ['ru', 'en', 'de', 'es']

def read_audio(path: str,
               sampling_rate: int = 16000):

    wav, sr = torchaudio.load(path)

    if wav.size(0) > 1:
        wav = wav.mean(dim=0, keepdim=True)

    if sr != sampling_rate:
        transform = torchaudio.transforms.Resample(orig_freq=sr,
                                                   new_freq=sampling_rate)
        wav = transform(wav)
        sr = sampling_rate

    assert sr == sampling_rate
    return wav.squeeze(0)


def save_audio(path: str,
               tensor: torch.Tensor,
               sampling_rate: int = 16000):
    torchaudio.save(path, tensor.unsqueeze(0), sampling_rate)


def init_jit_model(model_path: str,
                   device=torch.device('cpu')):
    torch.set_grad_enabled(False)
    model = torch.jit.load(model_path, map_location=device)
    model.eval()
    return model


def make_visualization(probs, step):
    import pandas as pd
    pd.DataFrame({'probs': probs},
                 index=[x * step for x in range(len(probs))]).plot(figsize=(16, 8),
                 kind='area', ylim=[0, 1.05], xlim=[0, len(probs) * step],
                 xlabel='seconds',
                 ylabel='speech probability',
                 colormap='tab20')


def get_speech_timestamps(audio: torch.Tensor,
                          model,
                          threshold: float = 0.5,
                          sampling_rate: int = 16000,
                          min_speech_duration_ms: int = 250,
                          min_silence_duration_ms: int = 100,
                          window_size_samples: int = 512,
                          speech_pad_ms: int = 30,
                          return_seconds: bool = False,
                          visualize_probs: bool = False):

    """
    This method is used for splitting long audios into speech chunks using silero VAD

    Parameters
    ----------
    audio: torch.Tensor, one dimensional
        One dimensional float torch.Tensor, other types are casted to torch if possible

    model: preloaded .jit silero VAD model

    threshold: float (default - 0.5)
        Speech threshold. Silero VAD outputs speech probabilities for each audio chunk, probabilities ABOVE this value are considered as SPEECH.
        It is better to tune this parameter for each dataset separately, but "lazy" 0.5 is pretty good for most datasets.

    sampling_rate: int (default - 16000)
        Currently silero VAD models support 8000 and 16000 sample rates

    min_speech_duration_ms: int (default - 250 milliseconds)
        Final speech chunks shorter min_speech_duration_ms are thrown out

    min_silence_duration_ms: int (default - 100 milliseconds)
        In the end of each speech chunk wait for min_silence_duration_ms before separating it

    window_size_samples: int (default - 1536 samples)
        Audio chunks of window_size_samples size are fed to the silero VAD model.
        WARNING! Silero VAD models were trained using 512, 1024, 1536 samples for 16000 sample rate and 256, 512, 768 samples for 8000 sample rate.
        Values other than these may affect model perfomance!!

    speech_pad_ms: int (default - 30 milliseconds)
        Final speech chunks are padded by speech_pad_ms each side

    return_seconds: bool (default - False)
        whether return timestamps in seconds (default - samples)

    visualize_probs: bool (default - False)
        whether draw prob hist or not

    Returns
    ----------
    speeches: list of dicts
        list containing ends and beginnings of speech chunks (samples or seconds based on return_seconds)
    """

    if not torch.is_tensor(audio):
        try:
            audio = torch.Tensor(audio)
        except:
            raise TypeError("Audio cannot be casted to tensor. Cast it manually")

    if len(audio.shape) > 1:
        for i in range(len(audio.shape)):  # trying to squeeze empty dimensions
            audio = audio.squeeze(0)
        if len(audio.shape) > 1:
            raise ValueError("More than one dimension in audio. Are you trying to process audio with 2 channels?")

    if sampling_rate > 16000 and (sampling_rate % 16000 == 0):
        step = sampling_rate // 16000
        sampling_rate = 16000
        audio = audio[::step]
        warnings.warn('Sampling rate is a multiply of 16000, casting to 16000 manually!')
    else:
        step = 1

    if sampling_rate == 8000 and window_size_samples > 768:
        warnings.warn('window_size_samples is too big for 8000 sampling_rate! Better set window_size_samples to 256, 512 or 768 for 8000 sample rate!')
    if window_size_samples not in [256, 512, 768, 1024, 1536]:
        warnings.warn('Unusual window_size_samples! Supported window_size_samples:\n - [512, 1024, 1536] for 16000 sampling_rate\n - [256, 512, 768] for 8000 sampling_rate')

    model.reset_states()
    min_speech_samples = sampling_rate * min_speech_duration_ms / 1000
    min_silence_samples = sampling_rate * min_silence_duration_ms / 1000
    speech_pad_samples = sampling_rate * speech_pad_ms / 1000

    audio_length_samples = len(audio)

    speech_probs = []
    for current_start_sample in range(0, audio_length_samples, window_size_samples):
        chunk = audio[current_start_sample: current_start_sample + window_size_samples]
        if len(chunk) < window_size_samples:
            chunk = torch.nn.functional.pad(chunk, (0, int(window_size_samples - len(chunk))))
        speech_prob = model(chunk, sampling_rate).item()
        speech_probs.append(speech_prob)

    print(len(speech_probs))

###################################
###################################

    triggered = False
    speeches = []
    current_speech = {}
    neg_threshold = threshold - 0.15
    temp_end = 0

    for i, speech_prob in enumerate(speech_probs):
        if (speech_prob >= threshold) and temp_end:
            temp_end = 0

        if (speech_prob >= threshold) and not triggered:
            triggered = True
            current_speech['start'] = window_size_samples * i
            continue

        if (speech_prob < neg_threshold) and triggered:
            if not temp_end:
                temp_end = window_size_samples * i
            if (window_size_samples * i) - temp_end < min_silence_samples:
                continue
            else:
                current_speech['end'] = temp_end
                if (current_speech['end'] - current_speech['start']) > min_speech_samples:
                    speeches.append(current_speech)
                temp_end = 0
                current_speech = {}
                triggered = False
                continue
    
    print(speeches)
    print(len(speeches))

    if current_speech and (audio_length_samples - current_speech['start']) > min_speech_samples:
        current_speech['end'] = audio_length_samples
        speeches.append(current_speech)

    for i, speech in enumerate(speeches):
        if i == 0:
            speech['start'] = int(max(0, speech['start'] - speech_pad_samples))
        if i != len(speeches) - 1:
            silence_duration = speeches[i+1]['start'] - speech['end']
            if silence_duration < 2 * speech_pad_samples:
                speech['end'] += int(silence_duration // 2)
                speeches[i+1]['start'] = int(max(0, speeches[i+1]['start'] - silence_duration // 2))
            else:
                speech['end'] = int(min(audio_length_samples, speech['end'] + speech_pad_samples))
                speeches[i+1]['start'] = int(max(0, speeches[i+1]['start'] - speech_pad_samples))
        else:
            speech['end'] = int(min(audio_length_samples, speech['end'] + speech_pad_samples))

    if return_seconds:
        for speech_dict in speeches:
            speech_dict['start'] = round(speech_dict['start'] / sampling_rate, 1)
            speech_dict['end'] = round(speech_dict['end'] / sampling_rate, 1)
    elif step > 1:
        for speech_dict in speeches:
            speech_dict['start'] *= step
            speech_dict['end'] *= step

    if visualize_probs:
        make_visualization(speech_probs, window_size_samples / sampling_rate)

    return speeches


def collect_chunks(tss: List[dict],
                   wav: torch.Tensor):
    chunks = []
    for i in tss:
        chunks.append(wav[i['start']: i['end']])
    return torch.cat(chunks)


def drop_chunks(tss: List[dict],
                wav: torch.Tensor):
    chunks = []
    cur_start = 0
    for i in tss:
        chunks.append((wav[cur_start: i['start']]))
        cur_start = i['end']
    return torch.cat(chunks)

def _detect_voice_activity(vad_model, audio):
        """Detect segments that have voice activities"""
        tic = time.time()

        speeches = get_speech_timestamps(
            audio, vad_model, sampling_rate=16000
        )

        # Remove too short segments
        # speeches = utils.remove_short_segments(speeches, 1.0 * self.sampling_rate)

        # # Expand to avoid to tight cut. You can tune the pad length
        # speeches = utils.expand_segments(
        #     speeches, 0.2 * self.sampling_rate, 0.0 * self.sampling_rate, audio.shape[0]
        # )

        # # Merge very closed segments
        # speeches = utils.merge_adjacent_segments(speeches, 0.5 * self.sampling_rate)

        print(f"Done voice activity detection in {time.time() - tic:.1f} sec")
        return speeches

# if __name__ == 'main':
vad_model = init_jit_model('silero_vad.jit')
audio = whisper.load_audio("/Users/devel0per/Code/audio-test/1.mp4", sr=16000)
speech_timestamps = _detect_voice_activity(vad_model, audio)
print(speech_timestamps)
print(len(speech_timestamps))