#![allow(unused_variables, dead_code)]

use eframe::egui::{self, Color32, ProgressBar};
// use eframe::{
//     App, Frame,
//     egui::{CentralPanel, Context},
// };
use crate::egui::RichText;
use egui::{FontData, FontDefinitions, FontFamily};
use egui_extras::install_image_loaders;
use egui_file::FileDialog;
use lofty::probe::Probe;
use rodio::{Decoder, OutputStream, Sink};
use rodio::{OutputStreamHandle, Source};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use lofty::file::{AudioFile, FileType, TaggedFileExt};
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
    bitrate: Option<u32>,
    file_type: Option<String>,
    opened_file: Option<PathBuf>,
    allowed_exts: Vec<&'static str>,
    open_file_dialog: Option<FileDialog>,
    error_message: Option<String>,
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

        let (_stream, stream_handle) = OutputStream::try_default()
            .unwrap_or_else(|e| panic!("Failed to initialize audio output: {e}"));
        let sink = Sink::try_new(&stream_handle)
            .unwrap_or_else(|e| panic!("Failed to create audio sink: {e}"));

        Self {
            _stream,
            stream_handle,
            sink,
            is_playing: false,
            total_duration: None,
            bitrate: None,
            file_type: None,
            opened_file: None,
            allowed_exts: vec!["m4a", "mp3", "flac"],
            open_file_dialog: None,
            error_message: None,
        }
    }

    fn load_file(&mut self, path: &Path) -> Result<(), String> {
        self.sink.stop();
        self.sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Failed to create sink: {e}"))?;

        let probe = Probe::open(path)
            .map_err(|e| format!("Failed to open file: {e}"))?
            .read()
            .map_err(|e| format!("Failed to read audio metadata: {e}"))?;

        self.file_type = Some(file_type_to_str(probe.file_type()).to_string());
        self.bitrate = probe.properties().overall_bitrate();

        let file = BufReader::new(File::open(path).map_err(|e| format!("File open error: {e}"))?);
        let source = Decoder::new(file).unwrap().track_position();
        self.total_duration = source.total_duration();
        self.sink.append(source);
        self.sink.pause();
        self.is_playing = false;

        Ok(())
    }
}

impl eframe::App for MyEguiApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
            ui.heading("Music Player");

            install_image_loaders(&ctx);

            if ui.button("\u{1F5C1} Open").clicked() {
                // Show only files with the allowed extensions.
                let filter = Box::new({
                    let allowed_exts: Vec<&'static str> =
                        self.allowed_exts.iter().copied().collect();
                    move |path: &Path| -> bool {
                        path.extension()
                            .and_then(OsStr::to_str)
                            .map(|ext| allowed_exts.iter().any(|&e| e.eq_ignore_ascii_case(ext)))
                            .unwrap_or(false)
                    }
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
                if let Err(e) = self.load_file(&path_buf) {
                    self.error_message = Some(e);
                };
            }

            ui.separator();

            ui.horizontal(|ui| {
                if let Some(path) = &self.opened_file {
                    match read_from_path(path) {
                        Ok(tagged_file) => {
                            if let Some(primary_tag) = tagged_file.primary_tag() {
                                // Try to show artwork if available
                                if let Some(picture) = primary_tag.pictures().first() {
                                    let icon_data = picture.data().to_vec();
                                    let texture_id = format!("icon-{}", path.to_string_lossy());
                                    ui.add_sized(
                                        (32.0, 32.0),
                                        egui::Image::from_bytes(texture_id, icon_data),
                                    );
                                }
                                ui.vertical(|ui| {
                                    if let Some(artist) =
                                        primary_tag.get_string(&lofty::tag::ItemKey::AlbumArtist)
                                    {
                                        ui.label(RichText::new(artist).color(Color32::DARK_GRAY));
                                    } else {
                                        ui.label("Unknown artist");
                                    }
                                    if let Some(title) =
                                        primary_tag.get_string(&lofty::tag::ItemKey::TrackTitle)
                                    {
                                        ui.label(title);
                                    } else {
                                        ui.label("Unknown title");
                                    }
                                });
                                // Try to show the track title if available
                            } else {
                                ui.label("No metadata available");
                            }
                        }
                        Err(err) => {
                            ui.colored_label(
                                egui::Color32::RED,
                                format!("Failed to read tags: {}", err),
                            );
                        }
                    }
                } else {
                    ui.label("No file selected.");
                }
            });

            ui.horizontal(|ui| {
                ui.label(self.file_type.clone().unwrap_or(String::from("Unknown")));
                ui.label(format!("{} kbps", self.bitrate.unwrap_or_default()));
            });

            let button_label = if self.is_playing == true {
                "⏸ Pause"
            } else {
                "▶ Play"
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
            });

            ui.horizontal(|ui| {
                let elapsed: Duration = self.sink.get_pos();
                let progress = if let Some(total) = self.total_duration {
                    if total.as_secs_f32() > 0.0 {
                        (elapsed.as_secs_f32() / total.as_secs_f32()).min(1.0)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                ui.add_sized((200.0, 20.0), ProgressBar::new(progress).show_percentage());
            });

            if let Some(err) = &self.error_message {
                ui.colored_label(Color32::RED, err);
            };
        });
    }
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}", minutes, seconds)
}

fn file_type_to_str(file_type: FileType) -> String {
    match file_type {
        FileType::Aac => String::from("aac"),
        FileType::Aiff => String::from("aiff"),
        FileType::Ape => String::from("ape"),
        FileType::Flac => String::from("flac"),
        FileType::Mpeg => String::from("mp3"),
        FileType::Mp4 => String::from("mp4"),
        FileType::Mpc => String::from("mpc"),
        FileType::Opus => String::from("opus"),
        FileType::Vorbis => String::from("ogg"),
        FileType::Speex => String::from("spx"),
        FileType::Wav => String::from("wav"),
        FileType::WavPack => String::from("wv"),
        _ => String::from("unknown"),
    }
}
