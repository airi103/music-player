#![allow(unused_variables, dead_code)]

use eframe::egui;
// use eframe::{
//     App, Frame,
//     egui::{CentralPanel, Context},
// };
use egui::{FontData, FontDefinitions, FontFamily};
use egui_extras::install_image_loaders;
use egui_file::FileDialog;
use rodio::{Decoder, OutputStream, Sink};
use rodio::{OutputStreamHandle, Source};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use lofty::file::TaggedFileExt;
use lofty::read_from_path;

// TODO: Add more robust error handling
fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Music Player",
        native_options,
        Box::new(|cc| Ok(Box::new(MyEguiApp::new(cc)))),
    )
    .unwrap();
}

struct MyEguiApp {
    _stream: OutputStream, // must be kept alive
    stream_handle: OutputStreamHandle,
    sink: Sink,
    is_playing: bool,
    total_duration: Option<Duration>,
    opened_file: Option<PathBuf>,
    open_file_dialog: Option<FileDialog>,
}

impl MyEguiApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        const NOTO_SANS_JP: &[u8] = include_bytes!("../assets/NotoSansJP-Regular.ttf");
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "noto_sans_jp".to_owned(),
            FontData::from_static(NOTO_SANS_JP).into(),
        );
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push("noto_sans_jp".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();

        Self {
            _stream,
            stream_handle,
            sink,
            is_playing: false,
            total_duration: None,
            opened_file: None,
            open_file_dialog: None,
        }
    }

    fn load_file(&mut self, path: &Path) {
        self.sink.stop();
        self.sink = Sink::try_new(&self.stream_handle).unwrap();

        let file = BufReader::new(File::open(path).unwrap());
        let source = Decoder::new(file).unwrap().track_position();
        self.total_duration = source.total_duration();
        self.sink.append(source);
        self.sink.pause();
        self.is_playing = false;
    }
}

impl eframe::App for MyEguiApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
            ui.heading("Music Player");

            install_image_loaders(&ctx);

            if (ui.button("Open")).clicked() {
                // Show only files with the extension "txt".
                let filter = Box::new({
                    let ext = Some(OsStr::new("m4a"));
                    move |path: &Path| -> bool { path.extension() == ext }
                });
                let mut dialog =
                    FileDialog::open_file(self.opened_file.clone()).show_files_filter(filter);
                dialog.open();
                self.open_file_dialog = Some(dialog);
            }

            if let Some(path_buf) = self.open_file_dialog.as_mut().and_then(|dialog| {
                if dialog.show(ctx).selected() {
                    dialog.path().map(|p| p.to_path_buf())
                } else {
                    None
                }
            }) {
                self.opened_file = Some(path_buf.clone());
                self.load_file(&path_buf);
            }

            ui.horizontal(|ui| {
                if let Some(path) = &self.opened_file {
                    if let Ok(tagged_file) = read_from_path(path) {
                        let icon = tagged_file
                            .primary_tag()
                            .unwrap()
                            .pictures()
                            .first()
                            .unwrap()
                            .data()
                            .to_vec();
                        let texture_id = format!(
                            "icon-{}",
                            self.opened_file
                                .as_ref()
                                .map(|p| p.to_string_lossy())
                                .unwrap_or_else(|| "default".into())
                        );
                        ui.add(egui::Image::from_bytes(texture_id, icon));
                        let title = tagged_file
                            .primary_tag()
                            .unwrap()
                            .get_string(&lofty::tag::ItemKey::TrackTitle)
                            .unwrap();
                        ui.label(title);
                    }
                } else {
                    ui.label("No file selected.");
                }
            });

            let button_label = if self.is_playing == true {
                "Pause"
            } else {
                "Play"
            };

            ui.horizontal(|ui| {
                if ui.button(button_label).clicked() {
                    if self.sink.is_paused() {
                        self.sink.play();
                        self.is_playing = true;
                    } else {
                        self.sink.pause();
                        self.is_playing = false;
                    }
                }
                let elapsed: Duration = self.sink.get_pos();
                ui.label(format!(
                    "{} / {}",
                    format_duration(elapsed),
                    format_duration(self.total_duration.unwrap_or_default())
                ));
            })
        });
    }
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}", minutes, seconds)
}
