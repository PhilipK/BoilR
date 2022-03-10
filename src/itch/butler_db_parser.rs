use nom::{
    bytes::complete::{tag, take_until},
    multi::many0,
    IResult,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DbPaths {
    pub(crate) base_path: String,
    pub(crate) path: String,
}

pub(crate) fn parse_butler_db<'a>(content: &'a [u8]) -> nom::IResult<&[u8], Vec<DbPaths>> {
    many0(parse_path)(content)
}

fn parse_path<'a>(i: &'a [u8]) -> nom::IResult<&[u8], DbPaths> {
    let prefix = "{\"basePath\":\"";
    let suffix = "\",\"totalSize\"";
    let (i, _taken) = take_until(prefix)(i)?;
    let (i, _taken) = tag(prefix)(i)?;
    let (i, base_path) = take_until(suffix)(i)?;
    let base_path = String::from_utf8_lossy(base_path).to_string();
    
    let prefix = ":[{\"path\":\"";
    let suffix = "\",\"depth";
    let (i, _taken) = take_until(prefix)(i)?;
    let (i, _taken) = tag(prefix)(i)?;
    let (i, path) = take_until(suffix)(i)?;
    let path = String::from_utf8_lossy(path).to_string();

    IResult::Ok((
        i,
        DbPaths {
            base_path,
            path,
        },
    ))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parse_itch_butler_db_test() {
        let content = include_bytes!("../testdata/itch/butler.db-wal");
        let result = parse_butler_db(content);
        assert!(result.is_ok());
        let (_r, paths) = result.unwrap();
        assert_eq!(paths.len(), 6);

        assert_eq!(paths[0].base_path, "/home/philip/.config/itch/apps/islands");
        assert_eq!(paths[0].path, "Islands_Linux.x86_64");
        assert_eq!(
            paths[1].base_path,
            "/home/philip/.config/itch/apps/night-in-the-woods"
        );
        assert_eq!(paths[1].path, "Night in the Woods.x86_64");
        assert_eq!(paths[2].base_path, "/home/philip/.config/itch/apps/islands");
        assert_eq!(paths[2].path, "Islands_Linux.x86_64");
        assert_eq!(
            paths[3].base_path,
            "/home/philip/.config/itch/apps/overland"
        );
        assert_eq!(paths[3].path, "Overland.x86_64");
        assert_eq!(
            paths[4].base_path,
            "/home/philip/.config/itch/apps/night-in-the-woods"
        );
        assert_eq!(paths[4].path, "Night in the Woods.x86_64");
        assert_eq!(paths[5].base_path, "/home/philip/.config/itch/apps/islands");
        assert_eq!(paths[5].path, "Islands_Linux.x86_64");
    }

    #[test]
    fn parse_itch_butler_db_test_other() {
        let content = include_bytes!("../testdata/itch/other-butler.db-wal");
        let result = parse_butler_db(content);
        assert!(result.is_ok());
        let (_r, paths) = result.unwrap();
        assert_eq!(paths.len(), 94);

        assert_eq!(paths[0].base_path, "/home/deck/.config/itch/apps/risetoruins");
        assert_eq!(paths[0].path, "Core.jar");
        //The parser finds douplicates 
        assert_eq!(paths[0], paths[1]);
        assert_eq!(paths[1], paths[2]);

        
    }
}
