use eframe::egui::{Align, InnerResponse, Layout, Response, Ui, Widget};

pub trait UiExtensions {
    fn fill_button(&mut self, widget: impl Widget) -> Response;
    fn column<R>(&mut self, width: f32, f: impl FnOnce(&mut Self)-> R) -> InnerResponse<R>;
}

impl UiExtensions for Ui {
    fn fill_button(&mut self, widget: impl Widget) -> Response {
        self.add_sized((self.available_width(), 20.0), widget)
    }

    fn column<R>(&mut self, width: f32, f: impl FnOnce(&mut Self) -> R) -> InnerResponse<R> {
        let third = (width, self.available_height()).into();
        self.allocate_ui_with_layout(third, Layout::top_down(Align::Max), f)
    }
}