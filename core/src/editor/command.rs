use std::collections::HashMap;

use ignore::Walk;

use crate::editor::menu::{Menu, MenuEntry};
use crate::editor::{Editor, EditorInfo, View};
use remote::protocol;

fn submenu_action(key: &str, editor: &mut Editor, client_id: usize) -> Result<(), failure::Error> {
    {
        let menu = editor.command_map.get_mut(key).unwrap();
        let info = EditorInfo {
            session: &editor.session_name,
            cwd: &editor.cwd,
            buffers: &editor.core.buffers(),
            views: &editor.core.views(),
        };
        menu.populate(&info);
    }
    let menu = &editor.command_map[key];
    editor.core.get_notifier().notify(
        client_id,
        protocol::notification::menu::new(menu.to_notification_params("")),
    );
    Ok(())
}

pub fn default_commands() -> HashMap<String, Menu> {
    let mut commands = HashMap::new();

    commands.insert(
        String::from(""),
        Menu::new("", "command", |_| {
            let mut entries = Vec::new();
            entries.push(MenuEntry {
                key: "open".to_string(),
                label: "Open file".to_string(),
                description: Some("Open and read a new file.".to_string()),
                action: submenu_action,
            });
            entries.push(MenuEntry {
                key: "quit".to_string(),
                label: "Quit".to_string(),
                description: Some("Quit the current client.".to_string()),
                action: |_key, editor, client_id| {
                    editor.command_quit(client_id)?;
                    Ok(())
                },
            });
            entries.push(MenuEntry {
                key: "scratch".to_string(),
                label: "New scratch".to_string(),
                description: Some("Create a new scratch buffer.".to_string()),
                action: submenu_action,
            });
            entries.push(MenuEntry {
                key: "view_select".to_string(),
                label: "Change view".to_string(),
                description: Some("Select an existing view or create a new one.".to_string()),
                action: submenu_action,
            });
            entries.push(MenuEntry {
                key: "view_delete".to_string(),
                label: "Delete view".to_string(),
                description: Some("Delete the current view.".to_string()),
                action: |_key, editor, client_id| {
                    editor.command_view_delete(client_id)?;
                    Ok(())
                },
            });
            entries.push(MenuEntry {
                key: "view_add".to_string(),
                label: "Add to view".to_string(),
                description: Some("Add another buffer to the current view.".to_string()),
                action: submenu_action,
            });
            entries.push(MenuEntry {
                key: "view_remove".to_string(),
                label: "Remove from the view.".to_string(),
                description: Some("Remove a buffer from the current view.".to_string()),
                action: submenu_action,
            });
            entries
        }),
    );

    commands.insert(
        String::from("open"),
        Menu::new("open", "file", |info| {
            Walk::new(&info.cwd)
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|ft| !ft.is_dir()).unwrap_or(false))
                .filter_map(|e| {
                    e.path()
                        .strip_prefix(&info.cwd)
                        .unwrap_or_else(|_| e.path())
                        .to_str()
                        .map(String::from)
                })
                .map(|fpath| MenuEntry {
                    key: fpath.to_string(),
                    label: fpath.to_string(),
                    description: None,
                    action: |key, editor, client_id| {
                        let mut path = editor.cwd.clone();
                        path.push(key);
                        let params = protocol::request::edit::Params {
                            file: key.to_owned(),
                            path: Some(path.into_os_string().into_string().unwrap()),
                            scratch: false,
                        };
                        editor.command_edit(client_id, &params)?;
                        Ok(())
                    },
                })
                .collect()
        }),
    );

    commands.insert(
        String::from("scratch"),
        Menu::prompt(
            "scratch",
            "scratch",
            "New scratch buffer name.",
            |key, editor, client_id| {
                let params = protocol::request::edit::Params {
                    file: key.to_owned(),
                    path: None,
                    scratch: true,
                };
                editor.command_edit(client_id, &params)?;
                Ok(())
            },
        ),
    );

    commands.insert(
        String::from("view_select"),
        Menu::new("view_select", "view", |info| {
            info.views
                .iter()
                .chain(info.buffers.iter().filter(|b| {
                    info.views
                        .iter()
                        .find(|&x| *x == View::for_buffer(b).key())
                        .is_none()
                }))
                .map(|key| MenuEntry {
                    key: key.to_string(),
                    label: key.to_string(),
                    description: None,
                    action: |key, editor, client_id| {
                        let params = protocol::request::view::Params {
                            view_id: key.to_owned(),
                        };
                        editor.command_view(client_id, &params)?;
                        Ok(())
                    },
                })
                .collect()
        }),
    );

    commands.insert(
        String::from("view_add"),
        Menu::new("view_add", "buffer", |info| {
            info.buffers
                .iter()
                .map(|buffer| MenuEntry {
                    key: buffer.to_string(),
                    label: buffer.to_string(),
                    description: None,
                    action: |key, editor, client_id| {
                        let params = protocol::request::view_add::Params {
                            buffer: key.to_owned(),
                        };
                        editor.command_view_add(client_id, &params)?;
                        Ok(())
                    },
                })
                .collect()
        }),
    );

    commands.insert(
        String::from("view_remove"),
        Menu::new("view_remove", "buffer", |info| {
            info.buffers
                .iter()
                .map(|buffer| MenuEntry {
                    key: buffer.to_string(),
                    label: buffer.to_string(),
                    description: None,
                    action: |key, editor, client_id| {
                        let params = protocol::request::view_remove::Params {
                            buffer: key.to_owned(),
                        };
                        editor.command_view_remove(client_id, &params)?;
                        Ok(())
                    },
                })
                .collect()
        }),
    );

    commands
}
