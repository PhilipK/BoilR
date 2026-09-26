use std::path::Path;

use nom::FindSubstring;

pub fn setup_proton_games<B: AsRef<str>>(games: &[B]) -> eyre::Result<()> {
    if let Ok(home) = std::env::var("HOME") {
        let config_file = Path::new(&home).join(".local/share/Steam/config/config.vdf");
        if config_file.exists() {
            if let Ok(config_content) = std::fs::read_to_string(&config_file) {
                let new_string = enable_proton_games(config_content, games);
                std::fs::write(config_file, new_string)?;
            }
        }
    }
    Ok(())
}

fn enable_proton_games<S: AsRef<str>, B: AsRef<str>>(vdf_content: S, games: &[B]) -> String {
    let vdf_content = vdf_content.as_ref();
    if let Some(section_info) = find_indexes(vdf_content) {
        let (base_indent_string, field_indent_string) = {
            let mut a = String::new();
            let mut b = String::new();
            for _i in 0..=section_info.base_indentation {
                a.push('\t');
                b.push('\t');
            }
            b.push('\t');
            (a, b)
        };

        let proton_replace_string = include_str!("proton_string.txt");
        let section_str = vdf_content.get(section_info.start..section_info.append_end);
        if let Some(section_str) = section_str {
            let games_strings_to_add = games
                .iter()
                .filter(|g| {
                    let game_section_start = format!("\"{}\"\n", g.as_ref());
                    !section_str.contains(&game_section_start)
                })
                .map(|game_id| {
                    let res = proton_replace_string.to_string();
                    let res = res.replace("\"X\"", &format!("\"{}\"", game_id.as_ref()));
                    let res = res.replace('=', &base_indent_string);
                    res.replace('+', &field_indent_string)
                });
            let mut new_section = section_str.to_string();
            for game_string in games_strings_to_add {
                new_section.push_str(&game_string);
            }
            new_section.push_str(&section_info.end_key);

            if let Some(before_section) = vdf_content.get(..section_info.start) {
                if let Some(after_section) = vdf_content.get(section_info.end..) {
                    return format!("{before_section}{new_section}{after_section}");
                }
            }
        }
    } else {
        //TODO make this an error instead?
        println!("Could not find proton section in steam, try to manually set proton on at least one game and then rerun");
    }
    vdf_content.to_string()
}

struct SectionInfo {
    start: usize,
    end: usize,
    append_end: usize,
    base_indentation: usize,
    end_key: String,
}

fn find_indexes<S: AsRef<str>>(vdf_content: S) -> Option<SectionInfo> {
    let compat_key = "\"CompatToolMapping\"\n";
    let vdf_content = vdf_content.as_ref();
    if let Some(compat_index) = vdf_content.find_substring(compat_key) {
        let compat_index = compat_index + compat_key.len();
        let after_key = vdf_content.get(compat_index..);
        if let Some(base_indentation) = after_key.and_then(|k| k.find('{')) {
            let mut end_key = "\n".to_string();
            for _i in 0..base_indentation {
                end_key.push('\t');
            }
            end_key.push('}');
            if let Some(end_index) = after_key.and_then(|a| a.find_substring(&end_key)) {
                return Some(SectionInfo {
                    start: compat_index,
                    end: compat_index + end_index + end_key.len(),
                    append_end: compat_index + end_index,
                    base_indentation,
                    end_key: end_key.to_string(),
                });
            }
        }
    }
    None
}

#[cfg(test)]
#[cfg(target_family = "unix")]
mod tests {

    //Okay to unwrap in tests
    #![allow(clippy::unwrap_in_result)]
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::indexing_slicing)]
    use super::*;

    #[test]
    pub fn can_find_index_test() {
        let input = include_str!("../testdata/vdf/testconfig.vdf");
        let SectionInfo {
            start,
            end,
            base_indentation,
            ..
        } = find_indexes(input).unwrap();

        let actual = input[start..end].to_string();
        let expected = include_str!("../testdata/vdf/compatmappingsection.vdf");
        assert_eq!(expected, actual);
        assert_eq!(4, base_indentation);
    }

    #[test]
    pub fn enable_proton_test() {
        let input = include_str!("../testdata/vdf/testconfig.vdf");
        let output = enable_proton_games(input, &["42", "43", "44"]);
        let expected = include_str!("../testdata/vdf/testconfig_expected.vdf");
        assert_eq!(expected, output);
    }

    #[test]
    pub fn enable_proton_test_empty() {
        let input = include_str!("../testdata/vdf/testconfig.vdf");
        let output = enable_proton_games(input, &["2719403116"]);
        let expected = include_str!("../testdata/vdf/testconfig.vdf");
        assert_eq!(expected, output);
    }
}
