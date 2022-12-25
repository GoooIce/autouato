use anyhow::Result;

pub fn normalize_timestamps(timestamps: &Vec<(i64, i64)>, end: i64) -> Vec<(i64, i64, bool)> {
    // timestamps = [(22560, 53216), (88064, 650720), (738304, 743904)];
    // end = 1000000
    // [(0, 22559, false),(22560, 53216, true), (53217, 88063, false), (88064, 650720, true), (650721, 738303, false), (738304, 743904, true), (743904, 1000000, false)]
    let mut normalized_timestamps: Vec<(i64, i64, bool)> = Vec::new();
    let mut last_end = 0;
    for (start, end) in timestamps {
        normalized_timestamps.push((last_end, start - 1, false));
        normalized_timestamps.push((*start, *end, true));
        last_end = end + 1;
    }
    normalized_timestamps.push((last_end, end, false));
    normalized_timestamps
}

pub fn ffmpeg_pipe(path: &str, timestamps: &Vec<(i64, i64)>, length: i64) -> Result<()> {
    let normalized_timestamps = normalize_timestamps(&timestamps, length);
    // ffmpeg -i input.mp4 -vf "setpts=PTS/2,split[main][aux];[aux]trim=0:5,setpts=PTS*2[fast];[main][fast]concat" output.mp4
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-i").arg(path);
    let mut filter = String::from("setpts=PTS/2,split[main][aux];");
    for (start, end, _is_speech) in normalized_timestamps {
        filter.push_str(&format!(
            "[aux]trim={},{}[fast{}];[main][fast{}]concat,",
            start, end, start, start
        ));
    }
    filter.pop();
    cmd.arg("-vf").arg(filter);
    cmd.arg("output.mp4");
    Ok(())
}
