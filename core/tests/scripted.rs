mod helpers;

use std::path::PathBuf;

use ced::script::exec_script_oneshot;
use ignore::Walk;

#[test]
fn run_all_scripts() {
    let scripts_root = helpers::root().join("core").join("tests").join("scripts");
    let mut scripts = Walk::new(scripts_root)
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().map(|ft| !ft.is_dir()).unwrap_or(false))
        .map(|e| e.into_path())
        .collect::<Vec<PathBuf>>();
    scripts.sort_unstable();

    let mut fails = 0;
    for script in scripts {
        let res = exec_script_oneshot(&script);
        if let Err(e) = res {
            println!("file: {}\n{}\n", script.display(), e);
            fails += 1;
        }
    }
    assert!(fails == 0, "some tests are failing");
}
