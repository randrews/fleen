mod fleen_app;

use std::fmt::format;
use std::path::{Path, PathBuf};
use eframe::egui::{Context, Id};
use eframe::{egui, Frame};
use egui_ltreeview::Action;
use crate::fleen_app::{FleenApp, FleenError, TreeEntry};

fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native("Fleen", native_options, Box::new(|cc| Ok(Box::new(FleenUi::default()))));
}

struct FleenUi {
    app: Option<FleenApp>,
    error: Option<FleenError>
}

impl Default for FleenUi {
    fn default() -> Self {
        Self { app: None, error: None }
    }
}

impl FleenUi {
    fn site_chooser(&mut self, ctx: &Context, frame: &mut Frame) {
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

    fn display(&mut self, ctx: &Context, frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                for action in self.tree_view(ui) {
                    if let Action::Activate(activate) = action {
                        for index in activate.selected {
                            self.app.as_ref().unwrap().open_file_at_index(index)
                        }
                    }
                }
            })
        });
    }

    fn tree_view(&mut self, ui: &mut egui::Ui) -> Vec<Action<usize>> {
        let (_, actions) = egui_ltreeview::TreeView::new(Id::from("tree")).show(ui, |builder| {
            for (id, entry) in self.app.as_mut().unwrap().file_tree_entries().into_iter().enumerate() {
                match entry {
                    TreeEntry::File(p) => builder.leaf(id, label_for_path(p)),
                    TreeEntry::Dir(p) => { builder.dir(id, label_for_path(p)); },
                    TreeEntry::CloseDir => builder.close_dir()
                }
            }
        });
        actions
    }
}

impl eframe::App for FleenUi {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        match &mut self.app {
            None => self.site_chooser(ctx, frame),
            Some(app) => self.display(ctx, frame)
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

fn label_for_path(path: PathBuf) -> String {
    path.file_name().unwrap().to_string_lossy().to_string()
}