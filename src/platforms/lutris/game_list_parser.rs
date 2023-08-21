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

        assert_eq!(games[0].id, 48);
    }

    #[test]
    fn reads_name() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(
            games[5].name,
            "Wolfenstein: The New Order"
        );
    }

    #[test]
    fn reads_id() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(
            games[5].slug,
            "wolfenstein_the_new_order"
        );
    }

    #[test]
    fn reads_platform() {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content);

        assert_eq!(games[1].service, "steam");
    }
}
