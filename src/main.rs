
use bevy::color::palettes::css::{BLACK, WHITE};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::{render::{render_asset::RenderAssetUsages, render_resource::{Extent3d, TextureDimension, TextureFormat}}, window::WindowResized};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use eulerian_fluid::{FluidScene, SceneType};
use bevy_mod_picking::prelude::*;

const WORLD_SIZE: (f32, f32) = (320.0, 180.0);

mod eulerian_fluid;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins.set(ImagePlugin::default_nearest()), EguiPlugin, DefaultPickingPlugins))
        .add_systems(Startup, setup_scene)
        .add_systems(FixedUpdate, update_fluid_simulation)
        .add_systems(Update, fit_window)
        .add_systems(Update, ui_system)
        .add_systems(PostUpdate, draw_scene_gizmos)
        .insert_resource(UiState {
            selected_scene: SceneType::WindTunnel,
        })
        .insert_resource(WindowInformation::default())
        .insert_resource(ObstacleInformation::default())
        .run();
}

#[derive(Resource, Default)]
struct WindowInformation {
    scale: (f32, f32),
}

#[derive(Resource, Default)]
struct ObstacleInformation {
    world_position: Vec2,
}

fn setup_scene(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn(Camera2dBundle::default());

    // Sets up an image
    let image = Image::new(
    Extent3d {
            width: WORLD_SIZE.0 as u32,
            height: WORLD_SIZE.1 as u32,
            ..default()
        },
        TextureDimension::D2,
        vec![0; (WORLD_SIZE.0 * WORLD_SIZE.1 * 4.) as usize],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    let image_handle = images.add(image);

    let mut fluid_scene = FluidScene::new(WORLD_SIZE.0, WORLD_SIZE.1, SceneType::WindTunnel);
    fluid_scene.image_handle = image_handle.clone();
    let pos = Vec2::new(
        (0. + (fluid_scene.width + 3.) / 2.) / fluid_scene.scale,
        (0. + (fluid_scene.height - 1.) / 2.) / fluid_scene.scale,
    );
    fluid_scene.set_obstacle(pos, true);

    commands.spawn(fluid_scene);

    commands.spawn((
        SpriteBundle {
            texture: image_handle.clone(),
            transform: Transform {
                scale: Vec3::new(1.0, 1.0, 1.0),
                translation: Vec3::new(0.0, 0.0, 1.0),
                ..Default::default()
            },
            ..Default::default()
        },
        On::<Pointer<Drag>>::run(|
            // Listener not actually needed
            _: Listener<Pointer<Drag>>,
            mut scene: Query<&mut FluidScene>,
            q_window: Query<&Window, With<PrimaryWindow>>,
            q_camera: Query<(&Camera, &GlobalTransform)>,
            mut obstacle_info: ResMut<ObstacleInformation>,
            | {
            let mut scene = scene.single_mut();

            // Getting world position
            let window = q_window.single();
            let (camera, camera_transform) = q_camera.single();
            if let Some(world_position) = window.cursor_position()
                .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor))
                .map(|ray| ray.origin.truncate())
            {
                obstacle_info.world_position = world_position;

                let pos = world_to_pos(world_position, &scene);

                scene.set_obstacle(pos, false);
            }
        }),
    ));
}

fn world_to_pos(world: Vec2, scene: &FluidScene) -> Vec2 {
    Vec2::new(
        (world.x + (scene.width + 3.) / 2.) / scene.scale,
        (world.y + (scene.height - 1.) / 2.) / scene.scale,
    )
}

fn pos_to_world(pos: Vec2, scene: &FluidScene) -> Vec2 {
    Vec2::new(
        pos.x - ((scene.width + 3.) / 2.),
        ((scene.height) / 2.) - pos.y
    )
}

fn pos_to_world_flip_y(pos: Vec2, scene: &FluidScene) -> Vec2 {
    Vec2::new(
        pos.x - ((scene.width + 3.) / 2.),
        pos.y - ((scene.height) / 2.)
    )
}

fn update_fluid_simulation(
    mut commands: Commands,
    mut query: Query<(Entity, &mut FluidScene)>,
    mut images: ResMut<Assets<Image>>,
    mut obstacle_info: ResMut<ObstacleInformation>,
    time: Res<Time>,
    ui_state: Res<UiState>,
) {
    for (entity, mut scene) in query.iter_mut() {
        let dt = time.delta_seconds();

        let image_data = images.get_mut(&scene.image_handle).unwrap().data.as_mut_slice();

        scene.step(dt, image_data);

        if ui_state.selected_scene != scene.scene_type {
            // Create a new scene
            commands.entity(entity).despawn();
            let mut new_scene = FluidScene::new(WORLD_SIZE.0, WORLD_SIZE.1, ui_state.selected_scene);

            let pos = Vec2::new(
                (0. + (scene.width + 3.) / 2.) / scene.scale,
                (0. + (scene.height - 1.) / 2.) / scene.scale,
            );
            new_scene.set_obstacle(pos, true);
            new_scene.image_handle = scene.image_handle.clone();
            commands.spawn(new_scene);

            obstacle_info.world_position = Vec2::ZERO;
        }
    }
}

fn draw_scene_gizmos(
    mut gizmos: Gizmos,
    scene: Query<&FluidScene>,
    obstacle_info: Res<ObstacleInformation>,
) {

    let scene = scene.single();

    let radius = scene.obstacle_radius + scene.fluid.h;

    let color = if scene.show_pressure && scene.show_smoke {
        WHITE
    } else {
        BLACK
    };

    gizmos.circle_2d(obstacle_info.world_position, scene.scale * radius, color);

    let fluid = &scene.fluid;
    if scene.show_velocities {
        let n = fluid.num_y;
        let h = fluid.h;

        for i in 0..fluid.num_x {
            for j in 0..fluid.num_y {
                let u = fluid.u[i * n + j];
                let v = fluid.v[i * n + j];

                // X arrow
                let y = scene.c_y((j as f32 + 0.5) * h, scene.height, scene.scale);
                let x0 = scene.c_x(i as f32 * h, scene.scale);
                let x1 = scene.c_x(i as f32 * h + u * 0.01, scene.scale);

                gizmos.arrow_2d(
                    pos_to_world(Vec2::new(x0, y), scene),
                    pos_to_world(Vec2::new(x1, y), scene),
                    BLACK
                );

                // Y arrow
                let x = scene.c_x((i as f32 + 0.5) * h, scene.scale);
                let y0 = scene.c_y(j as f32 * h, scene.height, scene.scale);
                let y1 = scene.c_y(j as f32 * h + v * 0.01, scene.height, scene.scale);

                gizmos.arrow_2d(
                    pos_to_world(Vec2::new(x, y0), scene),
                    pos_to_world(Vec2::new(x, y1), scene),
                    BLACK
                );
            }
        }
    }
    if scene.show_streamlines {
        let segment_length = fluid.h * 0.005;
        let segments = 3;
        for i in (1..(fluid.num_x - 1)).step_by(5) {
            for j in (1..(fluid.num_y - 1)).step_by(5) {
                let mut x = (i as f32 + 0.5) * fluid.h;
                let mut y = (j as f32 + 0.5) * fluid.h;

                for _ in 0..segments {
                    let u = fluid.sample_field(x, y, eulerian_fluid::Field::U);
                    let v = fluid.sample_field(x, y, eulerian_fluid::Field::V);
                    let l = f32::sqrt(u * u + v * v);
                    let mut x1 = x + (u / l * segment_length);
                    let mut y1 = y + (v / l * segment_length);

                    x1 += u * 0.01;
                    y1 += v * 0.01;
                    if x1 > fluid.num_x as f32 * fluid.h { break; }

                    gizmos.arrow_2d(
                        pos_to_world_flip_y((Vec2::new(x, y)) * scene.scale, scene),
                        pos_to_world_flip_y((Vec2::new(x1, y1)) * scene.scale, scene),
                        BLACK
                    );
                    x = x1;
                    y = y1;
                }
            }
        }
    }
}

// Scale the image to fit the window (integer scaling)
fn fit_window(
    mut resize_events: EventReader<WindowResized>,
    mut projections: Query<&mut OrthographicProjection>,
    mut window_info: ResMut<WindowInformation>,
) {
    for event in resize_events.read() {
        let h_scale = event.width / WORLD_SIZE.0 as f32;
        let v_scale = event.height / WORLD_SIZE.1 as f32;
        let mut projection = projections.single_mut();
        let new_scale = 1. / h_scale.min(v_scale).round();
        projection.scale = new_scale;

        window_info.scale = (event.width / WORLD_SIZE.0 as f32, event.height / WORLD_SIZE.1 as f32);
    }
}

// State for ui
#[derive(Resource)]
struct UiState {
    pub selected_scene: SceneType,
}

fn ui_system(
    mut contexts: EguiContexts,
    mut query: Query<&mut FluidScene>,
    mut ui_state: ResMut<UiState>,
) {
    let mut scene = query.single_mut();
    egui::Window::new("Configuration").title_bar(false).show(contexts.ctx_mut(), |ui| {

        ui.label("Simulation Types");
        let scene_type = &mut ui_state.selected_scene;
        egui::ComboBox::from_id_source("scene_type")
            .selected_text(format!("{:?}", scene_type))
            .show_ui(ui, |ui| {
                ui.selectable_value(scene_type, SceneType::WindTunnel, "Wind Tunnel");
                ui.selectable_value(scene_type, SceneType::HiresTunnel, "Hires Tunnel");
                ui.selectable_value(scene_type, SceneType::Tank, "Tank");
                ui.selectable_value(scene_type, SceneType::Paint, "Paint");
            });

        ui.label("Simulation Settings, (Depends on simulation type)");
        ui.checkbox(&mut scene.show_streamlines, "Show streamlines");
        ui.checkbox(&mut scene.show_velocities, "Show velocities");
        ui.checkbox(&mut scene.show_pressure, "Show pressure");
        ui.checkbox(&mut scene.show_smoke, "Show smoke");
        ui.checkbox(&mut scene.show_smoke_gradient, "Show smoke gradient");

        ui.separator();
        ui.label("Click and drag to move the obstacle");
    });
}