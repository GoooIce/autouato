// test ffmpeg pipe
// 2111165
// [(188448, 225248), (359936, 400864), (468480, 560096), (657920, 779744), (1025024, 1079264), (1115136, 1199072), (1349632, 1496032), (1529344, 1548768), (1882624, 1887200), (1939456, 1979360)]
use autouato::customer::service::normalize_timestamps;

// 将时间戳ts转换为00:00:00.000格式
fn format_time(t: i64) -> String {
    format!("{:.3}", t as f32 / 1000.0 / 16.0)
}

fn main() {
    let timestamps: Vec<(i64, i64)> = vec![(188448, 1979360)];

    let ts = normalize_timestamps(&timestamps, 2111165);

    assert_eq!(
        ts,
        vec![
            (0, 188447, false),
            (188448, 1979360, true),
            (1979361, 2111165, false)
        ]
    );

    let mut i = 0;

    // 将时间戳ts转换为00:00:00.000格式输出
    for (start, end, is_speech) in &ts {
        println!(
            "ffmpeg -ss {} -t {} -i input.mp4 -vcodec copy -acodec copy {}.mp4",
            format_time(*start),
            format_time(*end),
            i
        );
        if !*is_speech {
            println!(
                "ffmpeg -i {}.mp4 -filter:v 'setpts=0.25*PTS' -filter:a 'atempo=4.0' t{}.mp4",
                i, i
            );
        }
        i += 1;
    }

    println!("{:?}", ts);

    // ffmpeg -i input.mp4 -vf "setpts=PTS/2,split[main][aux];[aux]trim=0:5,setpts=PTS*2[fast];[main][fast]concat" output.mp4
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-i").arg("input.mp4");
    let mut filter = String::from("split");
    for (start, _end, _is_speech) in &ts {
        filter.push_str(&format!("[out{}]", start));
    }
    filter.push_str(";");
    for (start, end, _is_speech) in &ts {
        filter.push_str(&format!("[out{}]trim={}:{}", start, start, end));

        if *_is_speech {
            filter.push_str(";");
        } else {
            filter.push_str(&format!(",setpts=PTS*2[fast{}];", start));
        }
    }

    for (start, _end, _is_speech) in &ts {
        if *_is_speech {
            filter.push_str(&format!("[out{}]", start));
        } else {
            filter.push_str(&format!("[fast{}]", start));
        }
    }
    filter.push_str("concat");

    // filter.pop();
    cmd.arg("-vf").arg(filter);
    cmd.arg("output.mp4");
    println!("{:?}", cmd);
}
