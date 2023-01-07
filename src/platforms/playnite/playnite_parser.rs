use nom::{
    bytes::{
        complete::{tag, take_until, take_while},
        streaming::take,
    },
    character::is_alphanumeric,
    multi::many0,
    IResult,
};

pub(crate) struct NamesAndId {
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) installed: bool,
}

pub(crate) fn parse_db(content: &[u8]) -> nom::IResult<&[u8], Vec<NamesAndId>> {
    many0(parse_game)(content)
}

fn parse_game(i: &[u8]) -> nom::IResult<&[u8], NamesAndId> {
    let (i, _taken) = take_until("_id")(i)?;
    let (i, _taken) = take_until("Image")(i)?;
    let (i, prefix_and_id) = take_until("\\")(i)?;
    let id_bytes = prefix_and_id
        .split(|b| *b == 0_u8)
        .last()
        .unwrap_or_default();
    let id = String::from_utf8_lossy(id_bytes).to_string();
    let (i, _taken) = take_until("IsInstalled")(i)?;
    let (i, _taken) = tag("IsInstalled")(i)?;
    let installed = matches!(i.get(1), Some(1u8));
    let (i, _taken) = take_until("InstallSizeGroup")(i)?;
    let (i, _taken) = take_until("Name")(i)?;
    let (i, _taken) = take(4usize)(i)?;
    let (i, _taken) = take_while(|b| !is_alphanumeric(b))(i)?;
    let (i, name_bytes) = take_while(|b| b != 0)(i)?;
    let name = String::from_utf8_lossy(name_bytes).to_string();
    IResult::Ok((
        i,
        NamesAndId {
            id,
            name,
            installed,
        },
    ))
}
