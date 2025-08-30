use crate::mpeg_util::ChapterInfo;
use std::path::Path;

pub fn create_cue_sheet(file_path: &Path, chapters: &ChapterInfo) -> String {
    let mut cue_sheet = String::new();

    cue_sheet.push_str(&format!("FILE \"{}\" MP3\n", file_path.file_name().unwrap().to_string_lossy()));

    let mut track_count = 1;
    for _ in 0..chapters.count {
        // TODO: Get real chapter title and start time
        let title = format!("Chapter {}", track_count);
        let start_time_ms = 0; // placeholder
        let minutes = start_time_ms / 60000;
        let seconds = (start_time_ms % 60000) / 1000;
        let frames = (start_time_ms % 1000) * 75 / 1000;

        cue_sheet.push_str(&format!("  TRACK {:02} AUDIO\n", track_count));
        cue_sheet.push_str(&format!("    TITLE \"{}\"\n", title));
        cue_sheet.push_str(&format!("    INDEX 01 {:02}:{:02}:{:02}\n", minutes, seconds, frames));

        track_count += 1;
    }

    cue_sheet
}
