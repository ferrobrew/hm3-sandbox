use egui::CtxRef;

pub(crate) struct Console {
    text_input: String,
}

impl Console {
    pub(crate) fn new() -> Self {
        Self {
            text_input: Default::default(),
        }
    }

    pub fn render(&mut self, ctx: &CtxRef) {
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .min_height(200.0)
            .show(ctx, |ui| {
                ui.heading("Console");
                ui.separator();

                let text_style = egui::TextStyle::Body;
                let row_height = ui.fonts()[text_style].row_height();
                let num_rows = 6;
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show_rows(ui, row_height, num_rows, |ui, row_range| {
                        for row in row_range {
                            ui.label(format!("This is row {}/{}", row + 1, num_rows));
                        }
                    });
                ui.end_row();
                ui.add(
                    egui::TextEdit::singleline(&mut self.text_input)
                        .hint_text("Enter a command like 'exit' or `pause`"),
                );
            });
    }
}
