use anyhow::Result;
use std::io::Write;

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

// 将时间戳ts转换为00:00:00.000格式
fn format_time(t: i64) -> String {
    format!("{:.3}", t as f32 / 1000.0 / 16.0)
}

pub fn ffmpeg_pipe(path: &str, timestamps: &Vec<(i64, i64)>, length: i64) -> Result<()> {
    let normalized_timestamps = normalize_timestamps(&timestamps, length);
    let mut i = 0;
    let mut file_list: Vec<String> = Vec::new();

    // path获取目录路径并创建子目录tmp
    let mut tmp_path = std::path::PathBuf::from(path)
        .parent()
        .unwrap()
        .to_path_buf();
    tmp_path.push("tmp");
    std::fs::create_dir_all(&tmp_path)?;

    for (start, end, is_speech) in &normalized_timestamps {
        let mp4_name = if !*is_speech {
            format!("t{}.mp4", i)
        } else {
            format!("{}.mp4", i)
        };
        let binding = tmp_path.join(&mp4_name);
        let spilt_name = binding.as_path().to_str().unwrap();
        // 分割视频
        // ffmpeg -ss {} -t {} -i input.mp4 -vcodec copy -acodec copy {}.mp4
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.arg("-ss").arg(format_time(*start));
        cmd.arg("-t").arg(format_time(*end - *start));
        cmd.arg("-i").arg(path);
        cmd.arg("-vcodec").arg("copy");
        cmd.arg("-acodec").arg("copy");
        cmd.arg(spilt_name);
        println!("{:?}", cmd);
        let split_out = cmd.output()?;

        if split_out.status.success() {
            if !*is_speech {
                // 变速
                // ffmpeg -i {}.mp4 -filter:v 'setpts=0.5*PTS' -filter:a 'atempo=4.0' t{}.mp4
                let mut cmd = std::process::Command::new("ffmpeg");
                cmd.arg("-i").arg(spilt_name);
                cmd.arg("-filter:v").arg("setpts=0.5*PTS");
                cmd.arg("-filter:a").arg("atempo=2.0");
                cmd.arg(
                    tmp_path
                        .join(format!("{}.mp4", i))
                        .as_path()
                        .to_str()
                        .unwrap(),
                );
                cmd.output()?;

                // file_list.push(format!("t{}.mp4", i));
            }
            file_list.push(
                tmp_path
                    .join(format!("{}.mp4", i))
                    .as_path()
                    .to_str()
                    .unwrap()
                    .to_string(),
            );
        } else {
            println!("split failed");
        }

        i += 1;
    }

    // 输出合并项保存文件到{path}/tmp/merge.txt
    let binding = tmp_path.join("merge.txt");
    let txt_file_path = binding.as_path().to_str().unwrap();
    let mut txt_file = std::fs::File::create(txt_file_path)?;
    // file '{}'
    for file in &file_list {
        writeln!(txt_file, "file '{}'", file)?;
    }

    // 合并视频
    // ffmpeg -f concat -safe 0 -i input.txt -c copy output.mp4
    let binding = tmp_path.join("output.mp4");
    let output_file_path = binding.as_path().to_str().unwrap();
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-f").arg("concat");
    cmd.arg("-safe").arg("0");
    cmd.arg("-i").arg(txt_file_path);
    cmd.arg("-c").arg("copy");
    cmd.arg(output_file_path);
    cmd.output()?;
    Ok(())
}
