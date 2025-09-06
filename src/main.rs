mod fleen_app;
mod renderer;
mod server;
mod ui_ext;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use eframe::egui::{Button, Context, Direction, Id, Layout, RichText};
use eframe::{egui, Frame};
use egui_ltreeview::Action;
use tokio::task::JoinHandle;
use crate::fleen_app::{FileType, FleenApp, FleenError, TreeEntry};
use crate::server::start_server;
use crate::ui_ext::{ButtonExtensions, UiExtensions};

#[tokio::main]
async fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native("Fleen", native_options, Box::new(|_cc| {
        Ok(Box::new(FleenUi::default()))
    })).expect("Error running application");
}

enum DialogMode {
    NewFile(String),
    ConfirmDelete(String),
    RenameFile(String)
}

struct TempMessage {
    created: Instant,
    message: String
}

struct FleenUi {
    app: Option<FleenApp>,
    error: Option<FleenError>,
    selected_file: Option<String>,
    dialog_mode: Option<DialogMode>,
    server_handle: Option<JoinHandle<()>>,
    server_port: String,
    image_message: Option<TempMessage>
}

impl Default for FleenUi {
    fn default() -> Self {
        Self {
            app: None,
            error: None,
            selected_file: None,
            dialog_mode: None,
            server_handle: None,
            image_message: None,
            server_port: "3000".to_string(),
        }
    }
}

impl FleenUi {
    fn site_chooser(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let title = egui::Label::new(RichText::new("Select a site to manage").size(20.0));
            ui.vertical_centered(|ui| ui.add(title));

            if ui.add_fill_width(Button::blue("Open site...")).clicked() && let Some(path) = rfd::FileDialog::new().pick_folder() {
                match FleenApp::open(path.clone()) {
                    Ok(app) => {
                        self.app = Some(app);
                    }
                    Err(err) => { self.error = Some(err) }
                }
            }

            if ui.add_fill_width(Button::green("New site...")).clicked() && let Some(path) = rfd::FileDialog::new().pick_folder() {
                match FleenApp::create(path.clone()) {
                    Ok(app) => {
                        self.app = Some(app);
                        self.server_handle = Some(tokio::spawn(start_server(path, 3000)))
                    }
                    Err(err) => { self.error = Some(err) }
                }
            }
        });
    }

    fn display(&mut self, ctx: &Context) {
        let mut just_clicked = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            let width = ui.available_width() / 3.0 - 5.0;
            let height = ui.available_height() - 120.0;
            ui.horizontal(|ui| {
                ui.column(width, |ui| {
                    egui::ScrollArea::new([true, true])
                        .auto_shrink([false, false])
                        .min_scrolled_height(height)
                        .show(ui, |ui| self.tree_view(ui));
                    just_clicked = self.tree_buttons(ui);
                });
                ui.column(width, |ui| self.server_controls(ui));
                ui.column(width, |ui| {
                    if ui.add_fill_width(Button::green("Build site...")).clicked() &&
                        let Some(path) = rfd::FileDialog::new().pick_folder() {
                        match self.app.as_ref().unwrap().build_site(&path) {
                            // TODO some kind of temporary notification thing
                            Ok(()) => { println!("Yay!") }
                            Err(err) => { self.error = Some(err) }
                        }
                    }
                })
            });
        });

        match self.dialog_mode {
            Some(DialogMode::NewFile(_)) => self.new_file_dialog(ctx, just_clicked),
            Some(DialogMode::ConfirmDelete(_)) => self.confirm_delete_dialog(ctx),
            Some(DialogMode::RenameFile(_)) => self.rename_dialog(ctx, just_clicked),
            None => {}
        }
    }

    fn tree_view(&mut self, ui: &mut egui::Ui) {
        let tv = egui_ltreeview::TreeView::new(Id::from("tree"))
            .allow_multi_selection(false)
            .allow_drag_and_drop(false);
        let (_, actions) = tv.show(ui, |builder| {
            for entry in self.app.as_mut().unwrap().file_tree_entries().into_iter() {
                match entry {
                    TreeEntry::File(p) => builder.leaf(id_for_path(&p), label_for_path(&p)),
                    TreeEntry::Dir(p) => { builder.dir(id_for_path(&p), label_for_path(&p)); },
                    TreeEntry::CloseDir => builder.close_dir()
                }
            }
        });

        for action in actions {
            match action {
                Action::SetSelected(files) => {
                    self.selected_file = files.first().cloned()
                }
                Action::Activate(activate) => {
                    for fname in activate.selected {
                        self.handle_error(self.app.as_ref().unwrap().open_filename(&fname))
                    }
                }
                _ => {}
            }
        }
    }

    fn tree_buttons(&mut self, ui: &mut egui::Ui) -> bool {
        let mut just_clicked = false;

        if ui.add_fill_width(egui::Button::new("Open")).clicked() &&
            let Some(fname) = &self.selected_file {
            self.handle_error(self.app.as_ref().unwrap().open_filename(fname))
        }

        let new_btn = Button::green("New page");
        if ui.add_fill_width(new_btn).clicked() {
            just_clicked = true;
            self.dialog_mode = Some(DialogMode::NewFile(String::new()));
        }

        let rename_btn = Button::new("Rename");
        let delete_btn = Button::red("Delete");
        if self.root_selected() || self.selected_file.is_none() {
            ui.add_enabled_ui(false, |ui| {
                ui.add_fill_width(rename_btn);
                ui.add_fill_width(delete_btn);
            });
        } else if let Some(selected) = &self.selected_file {
            if ui.add_fill_width(rename_btn).clicked() {
                self.dialog_mode = Some(DialogMode::RenameFile(label_for_path(&PathBuf::from(&selected))));
                just_clicked = true;
            }
            if ui.add_fill_width(delete_btn).clicked() {
                self.dialog_mode = Some(DialogMode::ConfirmDelete(selected.clone()));
            }
        }

        ui.add_enabled_ui(self.app.as_ref().unwrap().image_dir_exists() && self.image_message.is_none(), |ui| {
            let label = match &self.image_message {
                Some(TempMessage { message, .. }) => message.as_str(),
                _ => "Image from clipboard"
            };

            if ui.add_fill_width(Button::blue(label)).clicked() {
                self.image_message = match self.app.as_ref().unwrap().paste_image() {
                    Ok(message) => Some(TempMessage { message, created: Instant::now() }),
                    Err(e) => Some(TempMessage { message: e.to_string(), created: Instant::now() })
                }
            }
        });
        just_clicked
    }

    fn server_controls(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.label("Port");
            let open_button = Button::new(format!("Open http://localhost:{}", self.server_port));
            let port_editor = egui::TextEdit::singleline(&mut self.server_port);

            if let Some(join_handle) = &self.server_handle {
                ui.add_enabled_ui(false, |ui| ui.add_fill_width(port_editor));
                let stop_btn = Button::red("Stop server");
                if ui.add_fill_width(stop_btn).clicked() {
                    join_handle.abort();
                    self.server_handle = None;
                }
                if ui.add_fill_width(open_button).clicked() {
                    self.app.as_ref().unwrap().open_server(self.server_port.as_str());
                }
            } else {
                ui.add(port_editor);
                let start_btn = Button::green("Start server");
                if let Ok(port_num) = self.server_port.parse::<u32>() {
                    if ui.add_fill_width(start_btn).clicked() {
                        let path = PathBuf::from(self.app.as_ref().unwrap().root_path());
                        self.server_handle = Some(tokio::spawn(start_server(path, port_num)))
                    }
                } else {
                    ui.add_enabled_ui(false, |ui| ui.add_fill_width(start_btn));
                }
                ui.add_enabled_ui(false, |ui| ui.add_fill_width(open_button));
            }
        });
    }

    fn new_file_dialog(&mut self, ctx: &Context, just_clicked: bool) {
        egui::Window::new("New page").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("Name");
            // If we press enter on the text field, it emits lost_focus. We can't just create the file based
            // on that, because it's not the only thing that causes it, though. So instead we'll just focus the
            // create file button
            //let fname = match &mut self.dialog_mode { Some(DialogMode::NewFile(s)) => s, _ => unreachable!() };
            let Some(DialogMode::NewFile(fname)) = &mut self.dialog_mode else { unreachable!() };
            let name_field = egui::TextEdit::singleline(fname);
            let resp = ui.add(name_field);
            let enter_key = resp.lost_focus();

            // We want to focus the text field if we just opened the dialog, but egui doesn't currently
            // offer a way to do that. However, we can pass in a bool for if the button that opens the
            // dialog was just clicked, and use that to tell whether we should grab focus. It's only true
            // the exact frame that the dialog was opened, which is what we're looking for.
            if just_clicked { resp.request_focus() }

            ui.horizontal(|ui| {
                let mut make_thing = |file_type: FileType| {
                    let Some(DialogMode::NewFile(fname)) = &self.dialog_mode else { unreachable!() };
                    let app = self.app.as_ref().unwrap();
                    let r = app.create_page(file_type,
                                            fname,
                                            self.selected_file.as_ref());
                    if r.is_err() {
                        self.handle_error(r);
                    } else {
                        self.dialog_mode = None; // Close the dialog, we're done
                    }
                };

                let btn = ui.button("New file");
                if enter_key { btn.request_focus() }
                if btn.clicked() { make_thing(FileType::File) }
                if ui.button("New directory").clicked() { make_thing(FileType::Dir) }
                if ui.button("Cancel").clicked() {
                    self.dialog_mode = None
                }
            })
        });
    }

    fn rename_dialog(&mut self, ctx: &Context, just_clicked: bool) {
        egui::Window::new("Rename").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("New name");
            let Some(DialogMode::RenameFile(fname)) = &mut self.dialog_mode else { unreachable!() };
            let name_field = egui::TextEdit::singleline(fname);
            let resp = ui.add(name_field);
            let enter_key = resp.lost_focus();
            if just_clicked { resp.request_focus() } // See new_file_dialog

            ui.horizontal(|ui| {
                let btn = ui.button("Rename");
                if enter_key { btn.request_focus() }
                if btn.clicked() {
                    let Some(DialogMode::RenameFile(fname)) = &self.dialog_mode else { unreachable!() };
                    let app = self.app.as_ref().unwrap();
                    let r = app.rename_page(self.selected_file.as_ref().unwrap(), fname);
                    if r.is_err() {
                        self.handle_error(r);
                    } else {
                        self.dialog_mode = None; // Close the dialog, we're done
                    }
                }
                if ui.button("Cancel").clicked() {
                    self.dialog_mode = None
                }
            })
        });
    }

    fn confirm_delete_dialog(&mut self, ctx: &Context) {
        let (mut del, mut cancel) = (false, false);
        let Some(DialogMode::ConfirmDelete(fname)) = &self.dialog_mode else { unreachable!() };
        egui::Window::new("Are you sure?").collapsible(false).resizable(false).show(ctx, |ui| {
            let file = label_for_path(&PathBuf::from(fname));
            ui.heading(format!("Really delete {}?", file));
            ui.horizontal(|ui| {
                let btn = Button::red("Yep, I'm sure");
                del = ui.add(btn).clicked();
                cancel = ui.button("Cancel").clicked();
            })
        });

        if del {
            let r = self.app.as_ref().unwrap().delete_page(&fname.clone());
            self.dialog_mode = None;
            if r.is_err() { self.handle_error(r); }
            self.selected_file = None;
        } else if cancel {
            self.dialog_mode = None
        }
    }

    fn handle_error(&mut self, result: Result<(), FleenError>) {
        if let Err(e) = result {
            self.error = Some(e)
        }
    }

    fn root_selected(&self) -> bool {
        if let Some(path) = &self.selected_file {
            path == &self.app.as_ref().unwrap().root_path()
        } else {
            false
        }
    }
}

impl eframe::App for FleenUi {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        match &mut self.app {
            None => self.site_chooser(ctx),
            Some(_) => self.display(ctx)
        }

        if let Some(err) = &self.error {
            let message = format!("{}", err);
            egui::Window::new("Error").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(message);
                if ui.button("I see").clicked() {
                    self.error = None
                }
            });
        }

        if let Some(TempMessage { created, .. }) = &self.image_message
            && *created < Instant::now() - Duration::from_secs(2) {
            self.image_message = None
        }
    }
}

fn label_for_path(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().to_string()
}

fn id_for_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}