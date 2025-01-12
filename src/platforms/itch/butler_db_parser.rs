use nom::{
    bytes::complete::{tag, take_until},
    multi::many0,
    IResult,
};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DbPaths {
    pub(crate) base_path: String,
    pub(crate) paths: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct Candidate {
    pub path: String,
}

pub(crate) fn parse_butler_db(content: &[u8]) -> nom::IResult<&[u8], Vec<DbPaths>> {
    many0(parse_path)(content)
}

fn parse_path(i: &[u8]) -> nom::IResult<&[u8], DbPaths> {
    let prefix = "{\"basePath\":\"";
    let suffix = "\",\"totalSize\"";
    let (i, _taken) = take_until(prefix)(i)?;
    let (i, _taken) = tag(prefix)(i)?;
    let (i, base_path) = take_until(suffix)(i)?;
    let base_path = String::from_utf8_lossy(base_path).to_string();

    let prefix = "\"candidates\":[";
    let suffix = "]}";
    let (i, _taken) = take_until(prefix)(i)?;
    let (i, _taken) = tag(prefix)(i)?;
    let (i, candidates_json) = take_until(suffix)(i)?;
    let candidates_json = format!("[{}]", String::from_utf8_lossy(candidates_json));

    let candidates = serde_json::from_str::<Vec<Candidate>>(&candidates_json);
    match candidates {
        Ok(candidates) => IResult::Ok((
            i,
            DbPaths {
                base_path,
                paths: candidates.iter().map(|c| c.path.clone()).collect(),
            },
        )),
        Err(_err) => {
            //we found a basepath, but no executables
            IResult::Ok((
                i,
                DbPaths {
                    base_path,
                    paths: vec![],
                },
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    //Okay to unwrap in tests
    #![allow(clippy::unwrap_in_result)]
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn parse_itch_butler_db_test() {
        let content = include_bytes!("../../testdata/itch/butler.db-wal");
        let result = parse_butler_db(content);
        assert!(result.is_ok());
        let (_r, paths) = result.unwrap();
        assert_eq!(paths.len(), 6);

        assert_eq!(paths[0].base_path, "/home/philip/.config/itch/apps/islands");
        assert_eq!(paths[0].paths[0], "Islands_Linux.x86_64");
        assert_eq!(
            paths[1].base_path,
            "/home/philip/.config/itch/apps/night-in-the-woods"
        );
        assert_eq!(paths[1].paths[0], "Night in the Woods.x86_64");
        assert_eq!(paths[2].base_path, "/home/philip/.config/itch/apps/islands");
        assert_eq!(paths[2].paths[0], "Islands_Linux.x86_64");
        assert_eq!(
            paths[3].base_path,
            "/home/philip/.config/itch/apps/overland"
        );
        assert_eq!(paths[3].paths[0], "Overland.x86_64");
        assert_eq!(
            paths[4].base_path,
            "/home/philip/.config/itch/apps/night-in-the-woods"
        );
        assert_eq!(paths[4].paths[0], "Night in the Woods.x86_64");
        assert_eq!(paths[5].base_path, "/home/philip/.config/itch/apps/islands");
        assert_eq!(paths[5].paths[0], "Islands_Linux.x86_64");
    }

    #[test]
    fn parse_itch_butler_db_test_other() {
        let content = include_bytes!("../../testdata/itch/other-butler.db-wal");
        let result = parse_butler_db(content);
        assert!(result.is_ok());
        let (_r, paths) = result.unwrap();
        assert_eq!(paths.len(), 94);

        assert_eq!(
            paths[0].base_path,
            "/home/deck/.config/itch/apps/risetoruins"
        );
        assert_eq!(paths[0].paths[0], "Core.jar");
        //The parser finds douplicates
        assert_eq!(paths[0], paths[1]);
        assert_eq!(paths[1], paths[2]);
    }
}
