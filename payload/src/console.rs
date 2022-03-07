use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
};

use chrono::{DateTime, Local};
use egui::{CtxRef, Ui};
use lazy_static::lazy_static;

// I hope you're happy, Josh, you're making me sell out my countrymen
type Color = (u8, u8, u8);
type Command = fn(&mut Console, &str, &[&str]) -> anyhow::Result<()>;
pub enum MessageType {
    Command,
    Info,
    Error,
    Misc(Color),
}

impl MessageType {
    fn color(&self) -> Color {
        match self {
            MessageType::Command => (243, 145, 137),
            MessageType::Info => (255, 255, 255),
            MessageType::Error => (249, 7, 22),
            MessageType::Misc(color) => *color,
        }
    }
}
const TIME_COLOR: Color = (255, 242, 204);

pub struct Console {
    text_input: String,
    messages: VecDeque<(DateTime<Local>, String, MessageType)>,
    commands: HashMap<String, Command>,
}

impl Console {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut console = Self {
            text_input: Default::default(),
            messages: VecDeque::with_capacity(1000),
            commands: HashMap::new(),
        };

        console.add_command("echo", |console, _, args| {
            console.push_back_message(args.join(" "), MessageType::Info);
            Ok(())
        });

        console
    }

    pub fn add_command(&mut self, cmd: &str, command: Command) {
        self.commands.insert(cmd.to_owned(), command);
    }

    pub fn push_back_message(&mut self, message: String, message_type: MessageType) {
        if self.messages.len() == self.messages.capacity() {
            let _ = self.messages.pop_front();
        }

        self.messages
            .push_back((Local::now(), message, message_type));
    }

    pub fn push_back_info(&mut self, message: String) {
        self.push_back_message(message, MessageType::Info);
    }

    pub fn push_back_error(&mut self, message: String) {
        self.push_back_message(message, MessageType::Error);
    }

    pub fn show(&mut self, ctx: &CtxRef) {
        egui::TopBottomPanel::bottom("console")
            .resizable(true)
            .min_height(200.0)
            .show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.heading("Console");
        ui.separator();

        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(false)
            .min_height(0.0)
            .show_inside(ui, |ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.text_input)
                            .hint_text("Enter a command like 'exit' or `pause`")
                            .desired_width(f32::INFINITY),
                    )
                    .lost_focus()
                {
                    self.enter_command();
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let text_style = egui::TextStyle::Body;
            let row_height = ui.fonts()[text_style].row_height();
            egui::ScrollArea::vertical().show_rows(
                ui,
                row_height,
                self.messages.len(),
                |ui, row_range| {
                    for row in row_range {
                        let (time, message, message_type) = &self.messages[row];
                        ui.horizontal_wrapped(|ui| {
                            {
                                let (r, g, b) = TIME_COLOR;
                                ui.colored_label(
                                    egui::Color32::from_rgb(r, g, b),
                                    time.format("%T").to_string(),
                                );
                            }
                            {
                                let (r, g, b) = message_type.color();
                                ui.colored_label(egui::Color32::from_rgb(r, g, b), message);
                            }
                        });
                    }
                },
            );
        });
    }

    fn enter_command(&mut self) {
        let input = self.text_input.clone();
        self.push_back_message(format!("> {}", input), MessageType::Command);
        self.text_input.clear();

        let words: Vec<_> = input.split_ascii_whitespace().collect();
        if words.is_empty() {
            self.push_back_error("Invalid command.".into());
            return;
        }

        let keyword = words[0];
        let arguments = &words[1..];

        if let Some(callback) = self.commands.get(keyword) {
            if let Err(err) = callback(self, keyword, arguments) {
                self.push_back_error(err.to_string());
            }
        } else {
            self.push_back_error(format!("The command `{}` does not exist.", keyword));
        }
    }
}

lazy_static! {
    pub static ref CONSOLE: Mutex<Console> = Mutex::new(Console::new());
}
