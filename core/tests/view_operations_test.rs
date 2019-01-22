use std::path::PathBuf;

mod helpers;

use ced::remote::protocol;

const CLIENT_ID: usize = 1;

fn p_file(name: &str, path: &PathBuf) -> protocol::request::edit::Params {
    protocol::request::edit::Params {
        file: String::from(name),
        path: Some(path.display().to_string()),
        scratch: false,
    }
}

fn p_scratch(name: &str) -> protocol::request::edit::Params {
    protocol::request::edit::Params {
        file: String::from(name),
        path: None,
        scratch: true,
    }
}

#[test]
fn add_and_remove() {
    let mut editor = helpers::SequentialEditor::new();
    editor.add_client(CLIENT_ID);
    editor.step();

    editor
        .command_edit(CLIENT_ID, &p_scratch("oh hi!"))
        .unwrap();
    editor.step();
    let mut file = helpers::root();
    file.push("Cargo.toml");
    editor
        .command_edit(CLIENT_ID, &p_file("Cargo.toml", &file))
        .unwrap();
    editor.step();

    editor
        .command_view_add(CLIENT_ID, &"oh hi!".into())
        .unwrap();
    editor.step();
    helpers::assert_buffers(
        &editor.state().view,
        vec![String::from("Cargo.toml"), String::from("oh hi!")],
    );

    editor
        .command_view_remove(CLIENT_ID, &"Cargo.toml".into())
        .unwrap();
    editor.step();
    helpers::assert_buffers(&editor.state().view, vec![String::from("oh hi!")]);
}

#[test]
fn delete() {
    let mut editor = helpers::SequentialEditor::new();
    editor.add_client(CLIENT_ID);
    editor.step();

    editor
        .command_edit(CLIENT_ID, &p_scratch("oh hi!"))
        .unwrap();
    editor.step();
    let mut file = helpers::root();
    file.push("Cargo.toml");
    editor
        .command_edit(CLIENT_ID, &p_file("Cargo.toml", &file))
        .unwrap();
    editor.step();

    helpers::assert_buffers(&editor.state().view, vec![String::from("Cargo.toml")]);

    editor.command_view_delete(CLIENT_ID).unwrap();
    editor.step();

    helpers::assert_buffers(&editor.state().view, vec![String::from("oh hi!")]);
}

#[test]
fn remove_and_delete() {
    let mut editor = helpers::SequentialEditor::new();
    editor.add_client(CLIENT_ID);
    editor.step();

    editor
        .command_edit(CLIENT_ID, &p_scratch("oh hi!"))
        .unwrap();
    editor.step();
    let mut file = helpers::root();
    file.push("Cargo.toml");
    editor
        .command_edit(CLIENT_ID, &p_file("Cargo.toml", &file))
        .unwrap();
    editor.step();

    helpers::assert_buffers(&editor.state().view, vec![String::from("Cargo.toml")]);

    editor
        .command_view_remove(CLIENT_ID, &"Cargo.toml".into())
        .unwrap();
    editor.step();

    helpers::assert_buffers(&editor.state().view, vec![String::from("oh hi!")]);
}
