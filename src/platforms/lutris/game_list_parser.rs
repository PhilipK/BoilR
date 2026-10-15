use super::lutris_game::LutrisGame;

/// An unreadable list is an error: it used to reach the user as "0 games".
pub fn parse_lutris_games(input: &str) -> eyre::Result<Vec<LutrisGame>> {
    if input.trim().is_empty() {
        return Ok(vec![]);
    }
    serde_json::from_str::<Vec<LutrisGame>>(input)
        .map_err(|e| eyre::eyre!("Could not read the game list from Lutris: {e}"))
}

#[cfg(test)]
mod tests {

    #![allow(clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn can_parse_output() -> eyre::Result<()> {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content)?;

        assert_eq!(6, games.len());
        Ok(())
    }

    #[test]
    fn empty_output_is_no_games() -> eyre::Result<()> {
        assert!(parse_lutris_games("\n")?.is_empty());
        Ok(())
    }

    #[test]
    fn unreadable_output_is_an_error() {
        assert!(parse_lutris_games("Traceback (most recent call last):").is_err());
    }

    #[test]
    fn reads_index() -> eyre::Result<()> {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content)?;

        assert_eq!(games[0].id, 48);
        Ok(())
    }

    #[test]
    fn reads_name() -> eyre::Result<()> {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content)?;

        assert_eq!(games[5].name, "Wolfenstein: The New Order");
        Ok(())
    }

    #[test]
    fn reads_id() -> eyre::Result<()> {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content)?;

        assert_eq!(games[5].slug, "wolfenstein_the_new_order");
        Ok(())
    }

    #[test]
    fn reads_platform() -> eyre::Result<()> {
        let content = include_str!("test_output.txt");

        let games = parse_lutris_games(content)?;

        assert_eq!(games[1].service.clone().unwrap_or_default(), "steam");
        Ok(())
    }
}
