mod helpers;

use std::path::PathBuf;

use async_std::task;
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

    let fails = task::block_on(async {
        let mut err_count = 0;
        let mut handles = Vec::new();
        for script in scripts {
            handles.push(task::spawn(async move {
                (script.clone(), exec_script_oneshot(&script))
            }));
        }

        for handle in handles {
            let (path, res) = handle.await;
            if let Err(e) = res {
                println!("file: {}\n{}\n", path.display(), e);
                err_count += 1;
            }
        }
        err_count
    });
    assert!(fails == 0, "some tests are failing");
}
