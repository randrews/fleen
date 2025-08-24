mod fleen_app;

use std::path::PathBuf;
use eframe::egui::{Color32, Context, Id, Stroke};
use eframe::{egui, Frame};
use egui_ltreeview::Action;
use crate::fleen_app::{FileType, FleenApp, FleenError, TreeEntry};

fn main() {
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

struct FleenUi {
    app: Option<FleenApp>,
    error: Option<FleenError>,
    selected_file: Option<String>,
    dialog_mode: Option<DialogMode>
}

impl Default for FleenUi {
    fn default() -> Self {
        Self {
            app: None,
            error: None,
            selected_file: None,
            dialog_mode: None
        }
    }
}

impl FleenUi {
    fn site_chooser(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("No site selected!");

            if ui.button("Open site...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    match FleenApp::open(path) {
                        Ok(app) => { self.app = Some(app) }
                        Err(err) => { self.error = Some(err) }
                    }
                }
            }

            if ui.button("New site...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    match FleenApp::create(path) {
                        Ok(app) => { self.app = Some(app) }
                        Err(err) => { self.error = Some(err) }
                    }
                }
            }
        });
    }

    fn display(&mut self, ctx: &Context) {
        let mut new_clicked = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    self.tree_view(ui);
                    if ui.add(egui::Button::new("Open")).clicked() {
                        if let Some(fname) = &self.selected_file {
                            self.handle_error(self.app.as_ref().unwrap().open_filename(fname))
                        }
                    }

                    if ui.add(egui::Button::new("New page")).clicked() {
                        new_clicked = true;
                        self.dialog_mode = Some(DialogMode::NewFile(String::new()));
                    }


                    let delete_btn = egui::Button::new("Delete").fill(Color32::DARK_RED);
                    if let Some(selected) = &self.selected_file {
                        if ui.add(delete_btn).clicked() {
                            self.dialog_mode = Some(DialogMode::ConfirmDelete(selected.clone()));
                        }
                    } else {
                        ui.add_enabled(false, delete_btn);
                    }
                });
            })
        });

        match self.dialog_mode {
            Some(DialogMode::NewFile(_)) => self.new_file_dialog(ctx, new_clicked),
            Some(DialogMode::ConfirmDelete(_)) => self.confirm_delete_dialog(ctx),
            Some(DialogMode::RenameFile(_)) => todo!(),
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
                    self.selected_file = files.first().map(String::clone)
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

    fn confirm_delete_dialog(&mut self, ctx: &Context) {
        let (mut del, mut cancel) = (false, false);
        let Some(DialogMode::ConfirmDelete(fname)) = &self.dialog_mode else { unreachable!() };
        egui::Window::new("Are you sure?").collapsible(false).resizable(false).show(ctx, |ui| {
            let file = label_for_path(&PathBuf::from(fname));
            ui.heading(format!("Really delete {}?", file));
            ui.horizontal(|ui| {
                let btn = egui::Button::new("Yep, I'm sure").fill(Color32::DARK_RED);
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
    }
}

fn label_for_path(path: &PathBuf) -> String {
    path.file_name().unwrap().to_string_lossy().to_string()
}

fn id_for_path(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}