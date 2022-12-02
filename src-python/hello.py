import datetime
import logging
import os
import time
import argparse

import opencc
import srt
import torch
import whisper

def is_video(filename):
    _, ext = os.path.splitext(filename)
    return ext in [".mp4", ".mov", ".mkv", ".flv"]


def change_ext(filename, new_ext):
    # Change the extension of filename to new_ext
    base, _ = os.path.splitext(filename)
    if not new_ext.startswith("."):
        new_ext = "." + new_ext
    return base + new_ext


def add_cut(filename):
    # Add cut mark to the filename
    base, ext = os.path.splitext(filename)
    if base.endswith("_cut"):
        base = base[:-4] + "_" + base[-4:]
    else:
        base += "_cut"
    return base + ext


def check_exists(output, force):
    if os.path.exists(output):
        if force:
            logging.info(f"{output} exists. Will overwrite it")
        else:
            logging.info(
                f"{output} exists, skipping... Use the --force flag to overwrite"
            )
            return True
    return False


def expand_segments(segments, expand_head, expand_tail, total_length):
    # Pad head and tail for each time segment
    results = []
    for i in range(len(segments)):
        t = segments[i]
        start = max(t["start"] - expand_head, segments[i - 1]["end"] if i > 0 else 0)
        end = min(
            t["end"] + expand_tail,
            segments[i + 1]["start"] if i < len(segments) - 1 else total_length,
        )
        results.append({"start": start, "end": end})
    return results


def remove_short_segments(segments, threshold):
    # Remove segments whose length < threshold
    return [s for s in segments if s["end"] - s["start"] > threshold]


def merge_adjacent_segments(segments, threshold):
    # Merge two adjacent segments if their distance < threshold
    results = []
    i = 0
    while i < len(segments):
        s = segments[i]
        for j in range(i + 1, len(segments)):
            if segments[j]["start"] < s["end"] + threshold:
                s["end"] = segments[j]["end"]
                i = j
            else:
                break
        i += 1
        results.append(s)
    return results


def compact_rst(sub_fn, encoding):
    cc = opencc.OpenCC("t2s")

    base, ext = os.path.splitext(sub_fn)
    COMPACT = "_compact"
    if ext != ".srt":
        logging.fatal("only .srt file is supported")

    if base.endswith(COMPACT):
        # to original rst
        with open(sub_fn, encoding=encoding) as f:
            lines = f.readlines()
        subs = []
        for l in lines:
            items = l.split(" ")
            if len(items) < 4:
                continue
            subs.append(
                srt.Subtitle(
                    index=0,
                    start=srt.srt_timestamp_to_timedelta(items[0]),
                    end=srt.srt_timestamp_to_timedelta(items[2]),
                    content=" ".join(items[3:]).strip(),
                )
            )
        with open(base[: -len(COMPACT)] + ext, "wb") as f:
            f.write(srt.compose(subs).encode(encoding, "replace"))
    else:
        # to a compact version
        with open(sub_fn, encoding=encoding) as f:
            subs = srt.parse(f.read())
        with open(base + COMPACT + ext, "wb") as f:
            for s in subs:
                f.write(
                    f"{srt.timedelta_to_srt_timestamp(s.start)} --> {srt.timedelta_to_srt_timestamp(s.end)} "
                    f"{cc.convert(s.content.strip())}\n".encode(encoding, "replace")
                )



class Transcribe:
    def __init__(self, args):
        self.args = args
        self.sampling_rate = 16000
        self.whisper_model = None
        self.vad_model = None
        self.detect_speech = None

    def run(self):
        for input in self.args.inputs:
            logging.info(f"Transcribing {input}")
            name, _ = os.path.splitext(input)
            if check_exists(name + ".md", self.args.force):
                continue

            audio = whisper.load_audio(input, sr=self.sampling_rate)
            if (self.args.vad == "1" or
                self.args.vad == "auto" and not name.endswith("_cut")):
                speech_timestamps = self._detect_voice_activity(audio)
            else:
                speech_timestamps = [{"start": 0, "end": len(audio)}]
            transcribe_results = self._transcribe(audio, speech_timestamps)

            output = name + ".srt"
            self._save_srt(output, transcribe_results)
            logging.info(f"Transcribed {input} to {output}")
            self._save_md(name + ".md", output, input)
            logging.info(f'Saved texts to {name + ".md"} to mark sentences')

    def _detect_voice_activity(self, audio):
        """Detect segments that have voice activities"""
        tic = time.time()
        if self.vad_model is None or self.detect_speech is None:
            # torch load limit https://github.com/pytorch/vision/issues/4156
            torch.hub._validate_not_a_forked_repo = lambda a, b, c: True
            self.vad_model, funcs = torch.hub.load(
                repo_or_dir="snakers4/silero-vad", model="silero_vad", trust_repo=True
            )

            self.detect_speech = funcs[0]

        speeches = self.detect_speech(
            audio, self.vad_model, sampling_rate=self.sampling_rate
        )

        # Remove too short segments
        speeches = remove_short_segments(speeches, 1.0 * self.sampling_rate)

        # Expand to avoid to tight cut. You can tune the pad length
        speeches = expand_segments(
            speeches, 0.2 * self.sampling_rate, 0.0 * self.sampling_rate, audio.shape[0]
        )

        # Merge very closed segments
        speeches = merge_adjacent_segments(speeches, 0.5 * self.sampling_rate)

        logging.info(f"Done voice activity detection in {time.time() - tic:.1f} sec")
        return speeches

    def _transcribe(self, audio, speech_timestamps):
        tic = time.time()
        if self.whisper_model is None:
            self.whisper_model = whisper.load_model(
                self.args.whisper_model, self.args.device
            )

        res = []
        # TODO, a better way is merging these segments into a single one, so whisper can get more context
        for seg in speech_timestamps:
            r = self.whisper_model.transcribe(
                audio[int(seg["start"]) : int(seg["end"])],
                task="transcribe",
                language=self.args.lang,
                initial_prompt=self.args.prompt,
            )
            r["origin_timestamp"] = seg
            res.append(r)
        logging.info(f"Done transcription in {time.time() - tic:.1f} sec")
        return res

    def _save_srt(self, output, transcribe_results):
        subs = []
        # whisper sometimes generate traditional chinese, explicitly convert
        cc = opencc.OpenCC("t2s")

        def _add_sub(start, end, text):
            subs.append(
                srt.Subtitle(
                    index=0,
                    start=datetime.timedelta(seconds=start),
                    end=datetime.timedelta(seconds=end),
                    content=cc.convert(text.strip()),
                )
            )

        prev_end = 0
        for r in transcribe_results:
            origin = r["origin_timestamp"]
            for s in r["segments"]:
                start = s["start"] + origin["start"] / self.sampling_rate
                end = min(
                    s["end"] + origin["start"] / self.sampling_rate,
                    origin["end"] / self.sampling_rate,
                )
                if start > end:
                    continue
                # mark any empty segment that is not very short
                if start > prev_end + 1.0:
                    _add_sub(prev_end, start, "< No Speech >")
                _add_sub(start, end, s["text"])
                prev_end = end

        with open(output, "wb") as f:
            f.write(srt.compose(subs).encode(self.args.encoding, "replace"))




def main():
    parser = argparse.ArgumentParser(
        description="Edit videos based on transcribed subtitles",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    logging.basicConfig(
        format="[autocut:%(filename)s:L%(lineno)d] %(levelname)-6s %(message)s"
    )
    logging.getLogger().setLevel(logging.INFO)

    parser.add_argument("inputs", type=str, nargs="+", help="Inputs filenames/folders")
    parser.add_argument(
        "-t",
        "--transcribe",
        help="Transcribe videos/audio into subtitles",
        action=argparse.BooleanOptionalAction,
    )

    parser.add_argument(
        "--lang",
        type=str,
        default="zh",
        choices=["zh", "en"],
        help="The output language of transcription",
    )
    parser.add_argument(
        "--prompt", type=str, default="", help="initial prompt feed into whisper"
    )
    parser.add_argument(
        "--whisper-model",
        type=str,
        default="small",
        choices=["tiny", "base", "small", "medium", "large"],
        help="The whisper model used to transcribe.",
    )
    parser.add_argument(
        "--bitrate",
        type=str,
        default="10m",
        help="The bitrate to export the cutted video, such as 10m, 1m, or 500k",
    )
    parser.add_argument(
        "--vad", help="If or not use VAD",
        choices=["1", "0", "auto"],
        default="auto"
    )
    parser.add_argument(
        "--force",
        help="Force write even if files exist",
        action=argparse.BooleanOptionalAction,
    )
    parser.add_argument(
        "--encoding", type=str, default="utf-8", help="Document encoding format"
    )
    parser.add_argument(
        "--device",
        type=str,
        default=None,
        choices=["cpu", "cuda"],
        help="Force to CPU or GPU for transcribing. In default automatically use GPU if available.",
    )

    args = parser.parse_args()

    if args.transcribe:
        Transcribe(args).run()

    else:
        logging.warn("No action, use -c, -t or -d")


if __name__ == "__main__":
    # main()
    print("hello world")
