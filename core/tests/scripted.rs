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

    task::block_on(async {
        let mut handles = Vec::new();
        for script in scripts {
            handles.push(task::spawn(async move {
                (script.clone(), exec_script_oneshot(&script))
            }));
        }

        for handle in handles {
            let (path, res) = handle.await;
            assert!(res.is_ok(), "{}\n> {}", path.display(), res.err().unwrap());
        }
    });
}
