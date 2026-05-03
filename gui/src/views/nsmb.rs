use std::{borrow::Cow, collections::BTreeSet};

use anyhow::Result;
use dsv_core::{gdb::client::GdbClient, state::State};
use eframe::egui::{self};

use crate::{
    client::{Client, Command},
    config::Config,
    util::read::{TypeInstance, TypeInstanceOptions},
    views::{read_object, read_pointer_object},
};

const PLAYER_ADDRESS: u32 = 0x0208b35c;
const SCENE_GRAPH_ADDRESS: u32 = 0x0208fb0c;

pub struct View {
    client: Client,
    windows: Windows,
}

struct Windows {
    basic_windows: [BasicWindow; 2],
    scene: SceneWindow,
}

impl View {
    pub fn new(gdb_client: GdbClient) -> Self {
        View { client: Client::new(gdb_client), windows: Windows::default() }
    }
}

impl Default for Windows {
    fn default() -> Self {
        Self {
            scene: SceneWindow { open: false },
            basic_windows: [
                BasicWindow {
                    open: false,
                    title: "Player",
                    type_name: "PlayerActor",
                    address: PLAYER_ADDRESS,
                    pointer: true,
                },
                BasicWindow {
                    open: false,
                    title: "SceneGraph",
                    type_name: "SceneGraph",
                    address: SCENE_GRAPH_ADDRESS,
                    pointer: false,
                },
            ],
        }
    }
}

impl super::View for View {
    fn render_side_panel(
        &mut self,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        _types: &type_crawler::Types,
        _config: &mut Config,
    ) -> Result<()> {
        egui::ScrollArea::vertical().max_width(100.0).show(ui, |ui| {
            ui.with_layout(
                egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true),
                |ui| {
                    ui.toggle_value(&mut self.windows.scene.open, "Scene");
                    for window in &mut self.windows.basic_windows {
                        ui.toggle_value(&mut window.open, window.title);
                    }
                },
            );
        });
        Ok(())
    }

    fn render_central_panel(
        &mut self,
        ctx: &egui::Context,
        _ui: &mut egui::Ui,
        types: &type_crawler::Types,
        config: &mut Config,
    ) -> Result<()> {
        let mut state = self.client.state.lock().unwrap();

        self.windows.scene.render(ctx, types, &mut state);

        for window in &mut self.windows.basic_windows {
            window.render(ctx, types, &mut state);
        }

        Ok(())
    }

    fn exit(&mut self) -> Result<()> {
        if !self.client.is_running() {
            return Ok(());
        }
        self.client.send_command(Command::Disconnect)?;
        self.client.join_update_thread();
        Ok(())
    }
}

#[derive(Default)]
struct BasicWindow {
    open: bool,
    title: &'static str,
    type_name: &'static str,
    address: u32,
    pointer: bool,
}

impl BasicWindow {
    fn render(&mut self, ctx: &egui::Context, types: &type_crawler::Types, state: &mut State) {
        let mut open = self.open;
        egui::Window::new(self.title).open(&mut open).resizable(true).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let object = if self.pointer {
                    read_pointer_object(types, state, self.type_name, self.address)
                } else {
                    read_object(types, state, self.type_name, self.address)
                };

                let instance = match object {
                    Ok(instance) => instance,
                    Err(err) => {
                        ui.label(err);
                        return;
                    }
                };
                instance.into_data_widget(ui, types).render_compound(ui, types, state);
            });
        });
        self.open = open;
    }
}

#[derive(Default)]
struct SceneWindow {
    open: bool,
}

impl SceneWindow {
    fn render(&mut self, ctx: &egui::Context, types: &type_crawler::Types, state: &mut State) {
        let mut open = self.open;
        egui::Window::new("Scene").open(&mut open).resizable(true).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let instance = match read_object(types, state, "SceneGraph", SCENE_GRAPH_ADDRESS) {
                    Ok(data) => data,
                    Err(err) => {
                        ui.label(err);
                        return;
                    }
                };
                let Some(root_pointer) = instance.read_int_field::<u32>(types, "root") else {
                    ui.label("ERR1");
                    return;
                };
                //ui.label(format!("{:x}", root_pointer));
                let root = match read_object(types, state, "SceneNode", root_pointer) {
                    Ok(data) => data,
                    Err(err) => {
                        ui.label(err);
                        return;
                    }
                };

                let Some(scene_pointer) = root.read_int_field::<u32>(types, "object") else {
                    ui.label("ERR2");
                    return;
                };
                //ui.label(format!("{:x}", scene_pointer));
                let scene = match read_object(types, state, "Scene", scene_pointer) {
                    Ok(data) => data,
                    Err(err) => {
                        ui.label(err);
                        return;
                    }
                };

                let Some(object_id) = scene.read_int_field::<u32>(types, "object_id") else {
                    ui.label("ERR3");
                    return;
                };
                let scene_type_name = get_object_class_name(object_id);
                let correct_scene = match read_object(types, state, scene_type_name, scene_pointer) {
                    Ok(data) => data,
                    Err(err) => {
                        ui.label(err);
                        return;
                    }
                };

                correct_scene.into_data_widget(ui, types).render_compound(ui, types, state);
            });
        });
        self.open = open;
    }
}

fn get_object_class_name(object_id: u32) -> &'static str {
    return match object_id {
        0 => "BootScene",
        1 => "Scene",
        2 => "DebugScene",
        3 => "StageScene",
        4 => "MainMenuScene",
        5..=17 => "Scene",
        _ => "Base",
    }
}