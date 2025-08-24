mod fleen_app;

use std::path::PathBuf;
use eframe::egui::{Color32, Context, Id, Stroke};
use eframe::{egui, Frame};
use egui_ltreeview::Action;
use crate::fleen_app::{FleenApp, FleenError, TreeEntry};

fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native("Fleen", native_options, Box::new(|_cc| {
        Ok(Box::new(FleenUi::default()))
    })).expect("Error running application");
}

struct FleenUi {
    app: Option<FleenApp>,
    error: Option<FleenError>,
    selected_file: Option<String>
}

impl Default for FleenUi {
    fn default() -> Self {
        Self {
            app: None,
            error: None,
            selected_file: None
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
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    self.tree_view(ui);
                    if ui.add(egui::Button::new("Open")).clicked() {
                        if let Some(fname) = &self.selected_file {
                            self.handle_error(self.app.as_ref().unwrap().open_filename(fname))
                        }
                    }
                    ui.add(egui::Button::new("New page")).clicked();
                });
            })
        });
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