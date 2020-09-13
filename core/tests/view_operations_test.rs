mod helpers;

use ced::remote::protocol::requests;

const CLIENT_ID: usize = 1;

fn p_file(name: &str) -> requests::EditParams {
    requests::EditParams {
        name: name.to_owned(),
        scratch: false,
    }
}

fn p_scratch(name: &str) -> requests::EditParams {
    requests::EditParams {
        name: name.to_owned(),
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
    editor
        .command_edit(CLIENT_ID, &p_file("Cargo.toml"))
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
    editor
        .command_edit(CLIENT_ID, &p_file("Cargo.toml"))
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
    editor
        .command_edit(CLIENT_ID, &p_file("Cargo.toml"))
        .unwrap();
    editor.step();

    helpers::assert_buffers(&editor.state().view, vec![String::from("Cargo.toml")]);

    editor
        .command_view_remove(CLIENT_ID, &"Cargo.toml".into())
        .unwrap();
    editor.step();

    helpers::assert_buffers(&editor.state().view, vec![String::from("oh hi!")]);
}
