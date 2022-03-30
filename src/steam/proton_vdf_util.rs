

pub fn enable_proton<S:AsRef<str>,B:AsRef<str>>(vdf_content:S ,games:&[B]) ->String{
    
    
    return vdf_content.as_ref().to_string();
}


#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn enable_proton_test(){
        let input = include_str!("../testdata/vdf/testconfig.vdf");
        let output = enable_proton(input,&vec!["42","43"]);
        let expected = include_str!("../testdata/vdf/testconfig_expected.vdf");
        assert_eq!(output,expected);

    }

}