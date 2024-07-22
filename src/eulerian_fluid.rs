use bevy::prelude::*;

const SIM_HEIGHT: f32 = 1.0;
const OBSTACLE_ZERO: Vec2 = Vec2::ZERO;
pub const OBSTACLE_RADIUS: f32 = 0.15;
const DENSITY: f32 = 1000.0;
const GRAVITY: f32 = -9.81;
const NUMBER_ITERATIONS: usize = 40;
const OVERRELAXATION: f32 = 1.9;
const TIMESTEP: f32 = 1.0 / 60.0;
const VELOCITY_IN: f32 = 2.0;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum SceneType {
    WindTunnel,
    HiresTunnel,
    Tank,
    Paint,
}

#[derive(Clone, Copy)]
pub enum Field {
    U,
    V,
    S,
}

#[derive(Component)]
pub struct FluidScene {
    pub gravity: f32,
    pub dt: f32,
    pub num_iters: usize,
    pub frame_nr: usize,
    pub over_relaxation: f32,
    pub obstacle_pos: Vec2,
    pub obstacle_radius: f32,
    pub show_streamlines: bool,
    pub show_velocities: bool,
    pub show_pressure: bool,
    pub show_smoke: bool,
    pub show_smoke_gradient: bool,

    pub scale: f32,
    pub width: f32,
    pub height: f32,

    pub fluid: Fluid,

    pub scene_type: SceneType,

    pub image_handle: Handle<Image>,
}

impl FluidScene {
    pub fn new(width: f32, height: f32, scene_type: SceneType) -> Self {
        let resolution: f32 = match scene_type {
            SceneType::Tank => 50.0,
            _ => 100.0,
        };

        let domain_height = SIM_HEIGHT;
        let domain_width = domain_height / height * width;
        let h = domain_height / resolution;
        let num_cells_x = f32::floor(domain_width / h) as usize + 2;
        let num_cells_y = f32::floor(domain_height / h) as usize + 2;

        let mut scene = FluidScene {
            gravity: GRAVITY,
            dt: TIMESTEP,
            num_iters: NUMBER_ITERATIONS,
            frame_nr: 0,
            over_relaxation: OVERRELAXATION,
            obstacle_pos: OBSTACLE_ZERO,
            obstacle_radius: OBSTACLE_RADIUS,
            show_streamlines: false,
            show_velocities: false,
            show_pressure: false,
            show_smoke: true,
            show_smoke_gradient: false,
            scale: height / domain_height,
            width,
            height,
            fluid: Fluid::new(DENSITY, num_cells_x, num_cells_y, h),
            scene_type,
            image_handle: Handle::default(),
        };

        match scene_type {
            SceneType::WindTunnel => scene.setup_tunnel(SceneType::WindTunnel),
            SceneType::HiresTunnel => scene.setup_tunnel(SceneType::HiresTunnel),
            SceneType::Tank => scene.setup_tank(),
            SceneType::Paint => scene.setup_paint(),
        };

        scene
    }

    pub fn step(&mut self, dt: f32, render_buffer: &mut [u8]) {
        self.dt = dt;

        self.fluid.integrate(dt, self.gravity);
        self.fluid.p.fill(0.0);

        self.fluid.solve_incompressibility(dt, self.num_iters as i32, self.over_relaxation);

        self.fluid.extrapolate();
        self.fluid.advect_vel(dt);
        self.fluid.advect_smoke(dt);

        self.frame_nr += 1;

        self.draw(render_buffer);
    }

    fn setup_tank(&mut self) {
        self.scene_type = SceneType::Tank;

        let fluid = &mut self.fluid;
        let n = fluid.num_y;
        for i in 0..fluid.num_x {
            for j in 0..fluid.num_y {
                let mut s = 1.0; // fluid
                if i == 0 || i == fluid.num_x - 1 || j == 0 {
                    s = 0.0; // solid
                }
                fluid.s[i * n + j] = s;
            }
        }

        self.gravity = -9.81;

        self.show_pressure = true;
        self.show_smoke = false;
        self.show_streamlines = false;
        self.show_velocities = false;
    }

    fn setup_tunnel(&mut self, scene_type: SceneType) {
        self.scene_type = scene_type;

        let fluid = &mut self.fluid;
        let n = fluid.num_y;
        for i in 0..fluid.num_x {
            for j in 0..fluid.num_y {
                let mut s = 1.0; // fluid
                if i == 0 || j == 0 || j == fluid.num_y - 1 {
                    s = 0.0; //solid
                }
                fluid.s[i * n + j] = s;
                if i == 1 {
                    fluid.u[i * n + j] = VELOCITY_IN;
                }
            }
        }

        let pipe_height = 0.1 * fluid.num_y as f32;
        let min_j = f32::floor(0.5 * fluid.num_y as f32 - 0.5 * pipe_height) as usize;
        let max_j = f32::floor(0.5 * fluid.num_y as f32 + 0.5 * pipe_height) as usize;

        for j in min_j..max_j {
            fluid.m[j] = 0.0; // solid
        }

        self.gravity = 0.0;

        self.show_smoke = true;

        if scene_type == SceneType::HiresTunnel {
            self.dt = 1.0 / 120.0;
            self.num_iters = 100;

            self.show_pressure = true;
        }
    }

    fn setup_paint(&mut self) {
        self.scene_type = SceneType::Paint;

        self.gravity = 0.0;
        self.over_relaxation = 1.0;
        self.obstacle_radius = 0.05;

        self.frame_nr = 0;

        self.show_smoke = true;
        self.show_smoke_gradient = true;
        self.show_pressure = false;
    }

    pub fn set_obstacle(&mut self, pos: Vec2, reset: bool) {
        if pos.x < 0.2 || pos.x > (self.width * 0.01) - 0.1 || pos.y < 0.1 || pos.y > (self.height * 0.01) - 0.1 {
            return;
        }
        let fluid = &mut self.fluid;

        let mut v = Vec2::ZERO;

        if !reset {
            v = (pos - self.obstacle_pos) / self.dt;
        }

        self.obstacle_pos = pos;
        let r = self.obstacle_radius;
        let n = fluid.num_y;
        let h = fluid.h;

        for i in 1..fluid.num_x - 2 {
            for j in 1..fluid.num_y - 2 {
                fluid.s[i * n + j] = 1.0;
                let dx = (i as f32 + 0.5) * h - pos.x;
                let dy = (j as f32 + 0.5) * h - pos.y;

                if dx * dx + dy * dy < r * r {
                    fluid.s[i * n + j] = 0.0;
                    fluid.m[i * n + j] = if self.scene_type == SceneType::Paint { 0.5 + 0.5 * f32::sin(0.1 * 2.0) } else { 1.0 };
                    fluid.u[i * n + j] = v.x;
                    fluid.u[(i + 1) * n + j] = v.x;
                    fluid.v[i * n + j] = v.y;
                    fluid.v[i * n + (j + 1)] = v.y;
                }
            }
        }
    }

    pub fn draw(&mut self, render_buffer: &mut [u8]) {
        let fluid = &self.fluid;

        let h = fluid.h;
        let cx = f32::floor(self.scale * h) as usize + 1;
        let cy = f32::floor(self.scale * h) as usize + 1;
        let n = fluid.num_y;

        let mut color = [255; 4];

        let mut p_min = fluid.p[0];
        let mut p_max = fluid.p[0];
        if self.show_pressure {
            for i in 0..fluid.num_cells {
                p_min = f32::min(p_min, fluid.p[i]);
                p_max = f32::max(p_max, fluid.p[i]);
            }
        }

        for i in 0..fluid.num_x {
            for j in 0..fluid.num_y {
                if self.show_pressure {
                    let p = fluid.p[i * n + j];
                    let s = fluid.m[i * n + j];
                    let sci_color = get_sci_color(p, p_min, p_max);
                    if self.show_smoke {
                        set_color(
                            &mut color,
                            &[
                                f32::max(0.0, sci_color[0] - 255.0 * s),
                                f32::max(0.0, sci_color[1] - 255.0 * s),
                                f32::max(0.0, sci_color[2] - 255.0 * s),
                            ],
                        );
                    } else {
                        set_color(&mut color, &sci_color);
                    }
                } else if self.show_smoke {
                    let s = fluid.m[i * n + j];
                    if self.show_smoke_gradient {
                        let sci_color = get_sci_color(s, 0.0, 1.0);
                        set_color(&mut color, &sci_color);
                    } else {
                        color_into_all(&mut color, 255.0 * s);
                    }
                } else if fluid.s[i * n + j] == 0.0 {
                    color[0..=2].fill(0);
                }
                let x = f32::floor(self.c_x((i as f32 - 1.0) * h, self.scale)) as usize;
                let y = f32::floor(self.c_y((j as f32 + 1.0) * h, self.height, self.scale)) as usize;
                for yi in y..y + cy {
                    let mut p = 4 * (yi * self.width as usize + x);
                    for _ in 0..cx {
                        p += 4;
                        if p <= render_buffer.len() {
                            render_buffer[p - 4..p].copy_from_slice(&color);
                        }
                    }
                }
            }
        }
    }

    pub fn c_x(&self, x: f32, scale: f32) -> f32 {
        x * scale
    }

    pub fn c_y(&self, y: f32, height: f32, scale: f32) -> f32 {
        height - y * scale
    }
}

pub struct Fluid {
    pub density: f32,
    pub num_x: usize,
    pub num_y: usize,
    pub num_cells: usize,
    pub h: f32,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
    pub new_u: Vec<f32>,
    pub new_v: Vec<f32>,
    pub p: Vec<f32>,
    pub s: Vec<f32>,
    pub m: Vec<f32>,
    pub new_m: Vec<f32>,
}

impl Fluid {
    fn new(density: f32, num_x: usize, num_y: usize, h: f32) -> Self {
        let num_cells = num_x * num_y;
        Fluid {
            density,
            num_x,
            num_y,
            num_cells,
            h,
            u: vec![0.0; num_cells],
            v: vec![0.0; num_cells],
            new_u: vec![0.0; num_cells],
            new_v: vec![0.0; num_cells],
            p: vec![0.0; num_cells],
            s: vec![0.0; num_cells],
            m: vec![1.0; num_cells], // Initially filled with 1.0
            new_m: vec![0.0; num_cells],
        }
    }

    fn integrate(&mut self, dt: f32, gravity: f32) {
        let n = self.num_y;
        for i in 1..self.num_x {
            for j in 1..(self.num_y - 1) {
                if self.s[i * n + j] != 0.0 && self.s[i * n + j - 1] != 0.0 {
                    self.v[i * n + j] += gravity * dt;
                }
            }
        }
    }

    fn solve_incompressibility(&mut self, dt: f32, iterations: i32, over_relaxation: f32) {
        let n = self.num_y;
        let cp = self.density * self.h / dt;
        for _ in 0..iterations {
            for i in 1..self.num_x - 1 {
                for j in 1..self.num_y - 1 {
                    if self.s[i * n + j] == 0.0 {
                        continue;
                    }

                    let sx0 = self.s[(i - 1) * n + j];
                    let sx1 = self.s[(i + 1) * n + j];
                    let sy0 = self.s[i * n + j - 1];
                    let sy1 = self.s[i * n + j + 1];
                    let s = sx0 + sx1 + sy0 + sy1;
                    if s == 0.0 {
                        continue;
                    }

                    let div = self.u[(i + 1) * n + j] -
                                   self.u[i * n + j] +
                                   self.v[i * n + j + 1] -
                                   self.v[i * n + j];
                    let p = (-div / s) * over_relaxation;
                    self.p[i * n + j] += cp * p;

                    self.u[i * n + j] -= sx0 * p;
                    self.u[(i + 1) * n + j] += sx1 * p;
                    self.v[i * n + j] -= sy0 * p;
                    self.v[i * n + j + 1] += sy1 * p;
                }
            }
        }
    }

    fn extrapolate(&mut self) {
        let n = self.num_y;
        for i in 0..self.num_x {
            self.u[i * n] = self.u[i * n + 1];
            self.u[i * n + n - 1] = self.u[i * n + n - 2];
        }
        for j in 0..self.num_y {
            self.v[j] = self.v[n + j];
            self.v[(self.num_x - 1) * n + j] = self.v[(self.num_x - 2) * n + j];
        }
    }

    pub fn sample_field(&self, x: f32, y: f32, field: Field) -> f32 {
        let n = self.num_y;
        let h = self.h;
        let h1 = 1.0 / h;
        let h2 = 0.5 * h;

        let x = x.clamp(h, self.num_x as f32 * h);
        let y = y.clamp(h, self.num_y as f32 * h);

        let mut dx = 0.0;
        let mut dy = 0.0;
        let f = match field {
            Field::U => {
                dy = h2;
                &self.u
            }
            Field::V => {
                dx = h2;
                &self.v
            }
            Field::S => {
                dx = h2;
                dy = h2;
                &self.m
            }
        };

        let x0 = f32::min(f32::floor((x - dx) * h1), (self.num_x - 1) as f32) as usize;
        let tx = ((x - dx) - x0 as f32 * h) * h1;
        let x1 = usize::min(x0 + 1, self.num_x - 1);

        let y0 = f32::min(f32::floor((y - dy) * h1), (self.num_y - 1) as f32) as usize;
        let ty = ((y - dy) - y0 as f32 * h) * h1;
        let y1 = usize::min(y0 + 1, self.num_y - 1);

        let sx = 1.0 - tx;
        let sy = 1.0 - ty;

        sx * sy * f[x0 * n + y0]
            + tx * sy * f[x1 * n + y0]
            + tx * ty * f[x1 * n + y1]
            + sx * ty * f[x0 * n + y1]
    }

    fn avg_u(&self, i: usize, j: usize) -> f32 {
        let n = self.num_y;
        (self.u[i * n + j - 1] +
             self.u[i * n + j] +
             self.u[(i + 1) * n + j - 1] +
             self.u[(i + 1) * n + j]) *
             0.25
    }

    fn avg_v(&self, i: usize, j: usize) -> f32 {
        let n = self.num_y;
        (self.v[(i - 1) * n + j] +
         self.v[i * n + j] +
         self.v[(i - 1) * n + j + 1] +
         self.v[i * n + j + 1]) *
         0.25
    }

    fn advect_vel(&mut self, dt: f32) {
        self.new_u.copy_from_slice(&self.u);
        self.new_v.copy_from_slice(&self.v);

        let n = self.num_y;
        let h = self.h;
        let h2 = 0.5 * h;

        for i in 0..self.num_x {
            for j in 0..self.num_y {
                // u component
                if self.s[i * n + j] != 0.0 &&
                    self.s[(i - 1) * n + j] != 0.0 &&
                    j < self.num_y - 1 
                {
                    let mut x = i as f32 * h;
                    let mut y = j as f32 * h + h2;
                    let mut u = self.u[i * n + j];
                    let v = self.avg_v(i, j);
                    x -= dt * u;
                    y -= dt * v;
                    u = self.sample_field(x, y, Field::U);
                    self.new_u[i * n + j] = u;
                }
                // v component
                if self.s[i * n + j] != 0.0 &&
                 self.s[i * n + j - 1] != 0.0 &&
                 i < self.num_x - 1
                {
                    let mut x = i as f32 * h + h2;
                    let mut y = j as f32 * h;
                    let u = self.avg_u(i, j);
                    let mut v = self.v[i * n + j];
                    x -= dt * u;
                    y -= dt * v;
                    v = self.sample_field(x, y, Field::V);
                    self.new_v[i * n + j] = v;
                }
            }
        }

        self.u.copy_from_slice(&self.new_u);
        self.v.copy_from_slice(&self.new_v);
    }

    fn advect_smoke(&mut self, dt: f32) {
        self.new_m.copy_from_slice(&self.m);

        let n = self.num_y;
        let h = self.h;
        let h2 = 0.5 * h;

        for i in 1..self.num_x - 1 {
            for j in 1..self.num_y - 1 {
                if self.s[i * n + j] != 0.0 {
                    let u = (self.u[i * n + j] + self.u[(i + 1) * n + j]) * 0.5;
                    let v = (self.v[i * n + j] + self.v[i * n + j + 1]) * 0.5;
                    let x = i as f32 * h + h2 - dt * u;
                    let y = j as f32 * h + h2 - dt * v;

                    self.new_m[i * n + j] = self.sample_field(x, y, Field::S);
                }
            }
        }
        self.m.copy_from_slice(&self.new_m);
    }
}

fn color_into_all(color: &mut [u8; 4], val: f32) {
    let val = f32::floor(val) as u8;
    color[0..=2].fill(val);
}

fn set_color(dest: &mut [u8; 4], src: &[f32; 3]) {
    dest[0] = f32::floor(src[0]) as u8;
    dest[1] = f32::floor(src[1]) as u8;
    dest[2] = f32::floor(src[2]) as u8;
}

pub fn get_sci_color(val: f32, min: f32, max: f32) -> [f32; 3] {
    let mut val = f32::min(f32::max(val, min), max - 0.0001);
    let d = max - min;
    val = if d == 0.0 { 0.5 } else { (val - min) / d };
    let m = 0.25;
    let num = f32::floor(val / m);
    let s = (val - num * m) / m;
    let (r, g, b) = match num as u8 {
        0 => (0.0, s, 1.0),
        1 => (0.0, 1.0, 1.0 - s),
        2 => (s, 1.0, 0.0),
        3 => (1.0, 1.0 - s, 0.0),
        _ => (1.0, 0.0, 0.0),
    };
    [255. * r, 255. * g, 255. * b]
}
