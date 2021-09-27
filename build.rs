#[cfg(feature = "ui")]
use std::env;
#[cfg(feature = "ui")]
use std::path::PathBuf;

fn main() {
    #[cfg(feature = "ui")]
    {
        println!("cargo:rerun-if-changed=gui/mainview.fl");
        let g = fl2rust::Generator::default();
        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        g.in_out(
            "gui/mainview.fl",
            out_path.join("mainview.rs").to_str().unwrap(),
        )
        .expect("Failed to generate rust from fl file!");
    }
}
