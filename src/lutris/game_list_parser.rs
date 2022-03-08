use super::lutris_game::LutrisGame;

pub fn parse_lutris_games<'a>(input: &'a str) -> Vec<LutrisGame> {
    input
        .split("\n")
        .into_iter()
        .filter(|s| !s.is_empty())
        .filter_map(parse_line)
        .collect()
}

fn parse_line<'a>(input: &'a str) -> Option<LutrisGame> {
    let mut sections = input.split("|");
    if sections.clone().count() < 4 {
        return None;
    }
    let index = sections.next().unwrap().trim();
    let name = sections.next().unwrap().trim();
    let id = sections.next().unwrap().trim();
    let platform = sections.next().unwrap().trim();

    Some(LutrisGame {
        id:id.to_string(),
        index:index.to_string(),
        name:name.to_string(),
        platform:platform.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse_output() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(19, games.len());
    }

    #[test]
    fn reads_index() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(games[0].index, "7");
    }

    #[test]
    fn reads_name() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(games[1].name, "Cave Story+");
    }

    #[test]
    fn reads_id() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(games[3].id, "dicey-dungeons");
    }

    #[test]
    fn reads_platform() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(games[18].platform, "steam");
    }
}
