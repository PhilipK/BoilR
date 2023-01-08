use super::lutris_game::LutrisGame;

pub fn parse_lutris_games(input: &str) -> Vec<LutrisGame> {
    let games = serde_json::from_str::<Vec<LutrisGame>>(input);
    match games {
        Ok(games) => games,
        Err(_err) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {

    #![allow(clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn can_parse_output() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(6, games.len());
    }

    #[test]
    fn reads_index() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(games[0].id, 1);
    }

    #[test]
    fn reads_name() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(
            games[5].name,
            "The Witcher 3: Wild Hunt - Game of the Year Edition"
        );
    }

    #[test]
    fn reads_id() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(
            games[5].slug,
            "the-witcher-3-wild-hunt-game-of-the-year-edition"
        );
    }

    #[test]
    fn reads_platform() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(games[1].runner, "steam");
    }
}
