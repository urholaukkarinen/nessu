#![deny(clippy::all)]

use std::fs::read;
use std::ops::Add;
use std::process::exit;
use std::time::{Duration, Instant};

use eframe::epaint::TextureHandle;
use eframe::{self, egui, CreationContext, Frame, NativeOptions, Theme};
use egui::panel::{Side, TopBottomSide};
use egui::{vec2, Color32, Context, CursorIcon, Event, Id, Key, Label, Sense, WidgetText};
use egui::{Ui, Widget};
use log::debug;

use nessu_lib::cartridge::Cartridge;
use nessu_lib::input::Button as NesButton;
use nessu_lib::nes::Nes;
use nessu_lib::op::{to_asm, CpuOpEntry, OpKind};

use crate::egui::{ColorImage, TextureFilter, Vec2};

const NES_DISPLAY_SIZE: [usize; 2] = [256, 240];
const APP_NAME: &str = "NESsu";

fn main() {
    #[cfg(feature = "logging")]
    pretty_env_logger::formatted_timed_builder()
        .filter_module("nessu_lib", log::LevelFilter::Debug)
        .try_init()
        .unwrap();

    eframe::run_native(
        APP_NAME,
        NativeOptions {
            initial_window_size: Some(Vec2::new(1600.0, 800.0)),
            default_theme: Theme::Dark,
            ..Default::default()
        },
        Box::new(|cc| Box::new(App::new(cc))),
    );
}

struct App {
    nes: Nes,
    running: bool,

    show_ppu_window: bool,
    show_cpu_window: bool,
    stop_execution_on_error: bool,

    update_scroll: bool,

    last_ft: Duration,

    display_texture: TextureHandle,
    nametable_textures: [TextureHandle; 4],

    next_frame_time: Instant,
    target_ft: Option<Duration>,

    loaded_cart_filename: Option<String>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        self.frame(ctx, frame);

        ctx.request_repaint();
    }
}

impl App {
    fn new(cc: &CreationContext) -> Self {
        let mut nes = Nes::new();
        nes.insert_cartridge(
            Cartridge::from_bytes(include_bytes!("../../../roms/snow.nes")).unwrap(),
        );

        let display_texture = cc.egui_ctx.load_texture(
            "display",
            ColorImage::new(NES_DISPLAY_SIZE, Color32::BLACK),
            TextureFilter::Nearest,
        );

        let nametable_textures = [
            cc.egui_ctx.load_texture(
                "nametable0",
                ColorImage::new(NES_DISPLAY_SIZE, Color32::BLACK),
                TextureFilter::Nearest,
            ),
            cc.egui_ctx.load_texture(
                "nametable1",
                ColorImage::new(NES_DISPLAY_SIZE, Color32::BLACK),
                TextureFilter::Nearest,
            ),
            cc.egui_ctx.load_texture(
                "nametable2",
                ColorImage::new(NES_DISPLAY_SIZE, Color32::BLACK),
                TextureFilter::Nearest,
            ),
            cc.egui_ctx.load_texture(
                "nametable3",
                ColorImage::new(NES_DISPLAY_SIZE, Color32::BLACK),
                TextureFilter::Nearest,
            ),
        ];

        Self {
            nes,
            running: true,
            show_ppu_window: false,
            show_cpu_window: true,
            stop_execution_on_error: true,
            last_ft: Duration::from_millis(0),
            display_texture,
            nametable_textures,
            next_frame_time: Instant::now(),
            target_ft: Some(Duration::from_nanos(16639263)),
            update_scroll: true,
            loaded_cart_filename: None,
        }
    }

    fn frame(&mut self, ctx: &Context, frame: &mut Frame) {
        if let Some(rom_name) = self.loaded_cart_filename.as_ref() {
            frame.set_window_title(&format!("{} ({})", APP_NAME, rom_name));
        }

        self.handle_dropped_file(ctx);
        self.handle_input(ctx);

        if self.running {
            while self.next_frame_time >= Instant::now() {}

            let start_time = Instant::now();
            self.next_frame_time = self
                .target_ft
                .map(|ft| start_time + ft)
                .unwrap_or(start_time);

            self.step_frame();

            self.last_ft = Instant::now().duration_since(start_time);
        } else {
            // finish any ongoing instruction
            while self.nes.cpu().instruction_ongoing() {
                self.nes.clock().ok();
            }

            let should_step = ctx
                .input()
                .events
                .iter()
                .any(|e| matches!(e, Event::Text(s) if s.as_str() == "."));

            if should_step {
                self.step_instruction();
            }
        }

        self.topbar(ctx);
        self.display_window(ctx);
        self.nametable_window(ctx);

        self.ppu_window(ctx);
        self.cpu_window(ctx);
        self.options_window(ctx);
    }

    fn load_cartridge(&mut self, name: &str, cartridge: Cartridge) {
        self.loaded_cart_filename = Some(name.to_string());
        self.nes.insert_cartridge(cartridge);
        self.update_scroll = true;
    }

    fn file_menu(&mut self, ui: &mut Ui) {
        ui.menu_button("File", |ui| {
            if ui.button("Reset").clicked() {
                self.nes.reset();
                ui.close_menu();
            }

            if ui.button("Power").clicked() {
                self.nes.power();
                ui.close_menu();
            }

            if ui.button("Quit").clicked() {
                exit(0);
            }
        });
    }

    fn view_menu(&mut self, ui: &mut Ui) {
        ui.menu_button("View", |ui| {
            if egui::Button::new("CPU").wrap(true).ui(ui).clicked() {
                self.show_cpu_window = !self.show_cpu_window;
                ui.close_menu();
            }

            if egui::Button::new("PPU").wrap(true).ui(ui).clicked() {
                self.show_ppu_window = !self.show_ppu_window;
                ui.close_menu();
            }
        });
    }

    fn ppu_window(&mut self, ctx: &Context) {
        egui::Window::new("PPU")
            .open(&mut self.show_ppu_window)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    egui::Grid::new("ppu_grid_1")
                        .striped(true)
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Cycle:");
                            ui.label(format!("{}", self.nes.ppu().current_cycle()));
                            ui.end_row();

                            ui.label("Scanline:");
                            ui.label(format!("{}", self.nes.ppu().current_scanline()));
                            ui.end_row();

                            ui.label("Scroll X:");
                            ui.label(format!("{}", self.nes.ppu().pixel_x));
                            ui.end_row();

                            ui.label("PPUCTRL:");
                            ui.label(format!("{:08b}", self.nes.ppu().ppu_ctrl));
                            ui.end_row();

                            ui.label("PPUMASK:");
                            ui.label(format!("{:08b}", self.nes.ppu().ppu_mask));
                            ui.end_row();

                            ui.label("PPUSTATUS:");
                            ui.label(format!("{:08b}", self.nes.ppu().ppu_status));
                            ui.end_row();
                        });
                });
            });
    }

    fn cpu_window(&mut self, ctx: &Context) {
        let mut show_cpu_window = self.show_cpu_window;
        egui::Window::new("CPU")
            .open(&mut show_cpu_window)
            .show(ctx, |ui| {
                egui::SidePanel::new(Side::Left, "ops")
                    .resizable(false)
                    .min_width(230.0)
                    .show_inside(ui, |ui| self.disassembly(ui));

                egui::SidePanel::new(Side::Right, "values")
                    .resizable(false)
                    .min_width(180.0)
                    .show_inside(ui, |ui| {
                        ui.vertical(|ui| {
                            egui::Grid::new("grid")
                                .striped(false)
                                .num_columns(2)
                                .show(ui, |ui| {
                                    ui.label("A:");
                                    ui.label(format!("{:02X}", self.nes.cpu().a));
                                    ui.end_row();

                                    ui.label("X:");
                                    ui.label(format!("{:02X}", self.nes.cpu().x));
                                    ui.end_row();

                                    ui.label("Y:");
                                    ui.label(format!("{:02X}", self.nes.cpu().y));
                                    ui.end_row();

                                    ui.label("P:");
                                    ui.label(format!("{:08b}", self.nes.cpu().p));
                                    ui.end_row();

                                    ui.label("PC:");
                                    ui.label(format!("${:04X}", self.nes.cpu().pc));
                                    ui.end_row();

                                    ui.separator();
                                    ui.end_row();

                                    ui.label("Cycles:");
                                    ui.label(format!(
                                        "{} (+{})",
                                        self.nes.cpu().cycles,
                                        self.nes.cpu().prev_op_cycles
                                    ));
                                    ui.end_row();
                                });

                            if ui.button("Step instruction").clicked() {
                                self.step_instruction();
                            }

                            if ui.button("Step frame").clicked() {
                                self.step_frame();
                            }
                        });
                    });
            });
        self.show_cpu_window = show_cpu_window;
    }

    fn options_window(&mut self, ctx: &Context) {
        egui::Window::new("Options").show(ctx, |ui| {
            egui::Grid::new("options_grid")
                .striped(true)
                .num_columns(1)
                .show(ui, |ui| {
                    ui.checkbox(
                        &mut self.nes.ppu_mut().sprite_rendering_enabled_by_user,
                        "Sprite rendering enabled",
                    );
                    ui.end_row();

                    ui.checkbox(
                        &mut self.nes.ppu_mut().bg_rendering_enabled_by_user,
                        "Background rendering enabled",
                    );
                    ui.end_row();

                    ui.checkbox(&mut self.stop_execution_on_error, "Stop execution on error");
                    ui.end_row();
                });
        });
    }

    fn step_instruction(&mut self) {
        if let Err(e) = self.nes.step_instruction() {
            eprintln!("{}", e);
            if self.stop_execution_on_error {
                self.running = false;
            }
        }
        self.update_scroll = true;
    }

    fn step_frame(&mut self) {
        if let Err(e) = self.nes.step_frame() {
            eprintln!("{}", e);
            if self.stop_execution_on_error {
                self.running = false;
            }
        }
        self.update_scroll = true;
    }

    fn disassembly(&mut self, ui: &mut Ui) {
        let text_style = egui::TextStyle::Body;
        let row_height = ui.text_style_height(&text_style);

        let mut scrollarea = egui::ScrollArea::vertical();
        let disassembly = self.nes.cpu_disassembly();

        if self.update_scroll {
            self.update_scroll = false;

            let idx = disassembly
                .iter()
                .enumerate()
                .find(|(_, op)| op.addr == self.nes.cpu().pc)
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            scrollarea = scrollarea.vertical_scroll_offset(
                (row_height + ui.spacing().item_spacing.y) * (idx.saturating_sub(10)) as f32,
            );

            if self.running {
                ui.spacing_mut().scroll_bar_width = 0.0;
            }
        }

        scrollarea.show_rows(ui, row_height, disassembly.len(), |ui, row_range| {
            ui.expand_to_include_rect(ui.available_rect_before_wrap());

            egui::Grid::new("disassembly")
                .striped(true)
                .min_col_width(60.0)
                .num_columns(3)
                .show(ui, |ui| {
                    ui.expand_to_include_rect(ui.available_rect_before_wrap());

                    for op_idx in row_range {
                        let CpuOpEntry {
                            addr,
                            opcode,
                            size,
                            kind,
                            addr_mode,
                            operands,
                        } = disassembly[op_idx as usize];

                        let active = addr == self.nes.cpu().pc;

                        ui.horizontal(|ui| {
                            if self.nes.cpu().is_breakpoint(addr) {
                                ui.painter().circle(
                                    ui.available_rect_before_wrap()
                                        .left_center()
                                        .add(vec2(7.0, 0.0)),
                                    4.0,
                                    Color32::RED,
                                    (0.0, Color32::TRANSPARENT),
                                );
                            }

                            ui.allocate_space(vec2(10.0, 0.0));

                            if self
                                .disassembly_label(ui, active, format!("{:04X}", addr))
                                .sense(Sense::click())
                                .ui(ui)
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.nes.cpu_mut().toggle_breakpoint(addr);
                            }
                        });

                        if kind == OpKind::Invalid {
                            self.disassembly_label(ui, active, format!("{:02X}", opcode))
                                .ui(ui);
                            self.disassembly_label(ui, active, "???").ui(ui);
                        } else if size == 2 {
                            self.disassembly_label(
                                ui,
                                active,
                                format!("{:02X} {:02X}", opcode, operands[0]),
                            )
                            .ui(ui);
                            self.disassembly_label(
                                ui,
                                active,
                                to_asm(kind, addr_mode, operands[0] as u16),
                            )
                            .ui(ui);
                        } else if size == 3 {
                            self.disassembly_label(
                                ui,
                                active,
                                format!("{:02X} {:02X} {:02X}", opcode, operands[0], operands[1]),
                            )
                            .ui(ui);
                            self.disassembly_label(
                                ui,
                                active,
                                to_asm(kind, addr_mode, u16::from_le_bytes(operands)),
                            )
                            .ui(ui);
                        } else {
                            self.disassembly_label(ui, active, format!("{:02X}", opcode))
                                .ui(ui);
                            self.disassembly_label(ui, active, to_asm(kind, addr_mode, 0))
                                .ui(ui);
                        }

                        ui.end_row();
                    }
                });
        });
    }

    fn disassembly_label<T>(&self, ui: &mut Ui, active: bool, text: T) -> Label
    where
        T: Into<WidgetText> + Into<String>,
    {
        if active {
            Label::new(
                egui::RichText::new(text).color(ui.style().visuals.widgets.active.fg_stroke.color),
            )
        } else {
            Label::new(text)
        }
    }

    fn display_window(&mut self, ctx: &Context) {
        self.display_texture.set(
            ColorImage::from_rgba_unmultiplied(NES_DISPLAY_SIZE, self.nes.display_bytes()),
            TextureFilter::Nearest,
        );

        egui::Window::new(format!(
            "Display ({:.02} ms)",
            self.last_ft.as_micros() as f32 / 1000.0
        ))
        .id(Id::new("display"))
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Image::new(self.display_texture.id(), [512.0, 480.0])
                .bg_fill(Color32::BLACK)
                .ui(ui);
        });
    }

    fn nametable_window(&mut self, ctx: &Context) {
        egui::Window::new("Nametables").show(ctx, |ui| {
            for i in 0..4 {
                self.nametable_textures[i].set(
                    ColorImage::from_rgba_unmultiplied(
                        NES_DISPLAY_SIZE,
                        &self.nes.nametable_rgb_bytes(i as _),
                    ),
                    TextureFilter::Nearest,
                );
            }

            ui.horizontal(|ui| {
                self.nametable_image(0).ui(ui);
                self.nametable_image(1).ui(ui);
            });
            ui.horizontal(|ui| {
                self.nametable_image(2).ui(ui);
                self.nametable_image(3).ui(ui);
            });
        });
    }

    fn nametable_image(&mut self, idx: usize) -> egui::Image {
        egui::Image::new(self.nametable_textures[idx].id(), [256.0, 240.0]).bg_fill(Color32::BLACK)
    }

    fn topbar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::new(TopBottomSide::Top, "topbar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                self.file_menu(ui);
                self.view_menu(ui);
            })
        });
    }

    fn handle_input(&mut self, ctx: &Context) {
        let input = ctx.input();

        if input.key_pressed(Key::Space) {
            self.running = !self.running;
        }

        self.nes
            .set_button_state_player1(NesButton::Down, input.key_down(Key::ArrowDown));
        self.nes
            .set_button_state_player1(NesButton::Up, input.key_down(Key::ArrowUp));
        self.nes
            .set_button_state_player1(NesButton::Left, input.key_down(Key::ArrowLeft));
        self.nes
            .set_button_state_player1(NesButton::Right, input.key_down(Key::ArrowRight));
        self.nes
            .set_button_state_player1(NesButton::Start, input.key_down(Key::Enter));
        self.nes
            .set_button_state_player1(NesButton::Select, input.key_down(Key::S));
        self.nes
            .set_button_state_player1(NesButton::A, input.key_down(Key::A));
        self.nes
            .set_button_state_player1(NesButton::B, input.key_down(Key::B));
    }

    fn handle_dropped_file(&mut self, ctx: &Context) {
        for file in ctx.input().raw.dropped_files.iter() {
            debug!("{:?}", file);
            if let Some(path) = file.path.as_ref() {
                let bytes = read(path).unwrap();

                if let Ok(cartridge) = Cartridge::from_bytes(&bytes) {
                    self.load_cartridge(path.file_name().unwrap().to_str().unwrap(), cartridge);
                }
            }
        }
    }
}
