use crate::mpeg_util::ChapterInfo;
use std::path::Path;

pub fn create_cue_sheet(file_path: &Path, chapters: &ChapterInfo) -> String {
    let mut cue_sheet = String::new();

    cue_sheet.push_str(&format!("FILE \"{}\" MP3\n", file_path.file_name().unwrap().to_string_lossy()));

    let mut track_count = 1;
    for chapter in &chapters.chapters {
        let total_seconds = chapter.start_time.seconds;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        let frames = (chapter.start_time.frac * 75.0) as u64;

        cue_sheet.push_str(&format!("  TRACK {:02} AUDIO\n", track_count));
        cue_sheet.push_str(&format!("    TITLE \"{}\"\n", chapter.title));
        cue_sheet.push_str(&format!("    INDEX 01 {:02}:{:02}:{:02}\n", minutes, seconds, frames));

        track_count += 1;
    }

    cue_sheet
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpeg_util::Chapter;
    use symphonia::core::units::Time;

    #[test]
    fn test_create_cue_sheet() {
        let chapters = ChapterInfo {
            chapters: vec![
                Chapter {
                    title: "Chapter 1".to_string(),
                    start_time: Time::new(0, 0.0),
                },
                Chapter {
                    title: "Chapter 2".to_string(),
                    start_time: Time::new(60, 0.0),
                },
            ],
        };
        let cue_sheet = create_cue_sheet(Path::new("test.mp3"), &chapters);
        let expected = "FILE \"test.mp3\" MP3\n  TRACK 01 AUDIO\n    TITLE \"Chapter 1\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"Chapter 2\"\n    INDEX 01 01:00:00\n";
        assert_eq!(cue_sheet, expected);
    }
}
