//! Generate the default world, rain on it, and write one side-on PNG.
//!
//! `cargo run -p cubarium-voxel --example basin` — a few hundred ticks, a ledger line
//! every 50, and `target/voxel-basin.png` at the end.

use std::fs::File;
use std::io::BufWriter;

use cubarium_voxel::{Command, Config, Material, World, TICK_HZ};

const TICKS: u32 = 300;
const SCALE: usize = 4;

fn main() -> anyhow::Result<()> {
    let config = Config {
        rain_m_per_s: 0.002,
        evaporation_m_per_s: 0.0004,
        ..Config::default()
    };
    let mut world = World::new(config);
    let c = world.config().clone();
    println!(
        "world {}x{}x{} at {} m/voxel, seed {}, outlet {:?}, spring {:?}",
        c.width,
        c.height,
        c.depth,
        c.voxel_m,
        c.seed,
        world.outlet_cell(),
        world.spring_cell()
    );

    world.apply(Command::ChargeAquifer { volume_m3: 80.0 });
    let pulse = world.apply(Command::RainPulse { volume_m3: 30.0 });
    println!("rain pulse accepted {pulse:.3} m3, aquifer head {:.2} m", world.aquifer_head_m());

    for tick in 1..=TICKS {
        if tick == TICKS / 2 {
            world.apply(Command::SetOutlet { open: true });
            println!("-- outlet opened at tick {tick}");
        }
        world.step();
        if tick % 50 == 0 {
            report(&world);
        }
    }
    report(&world);

    let path = "target/voxel-basin.png";
    std::fs::create_dir_all("target")?;
    write_png(&world, path)?;
    println!(
        "wrote {path} ({}x{} px, {:.1} s of world time)",
        c.width as usize * SCALE,
        c.height as usize * SCALE,
        TICKS as f64 / TICK_HZ as f64
    );
    Ok(())
}

fn report(world: &World) {
    let v = world.view();
    let l = v.ledger;
    let (free, pore) = stores(world);
    println!(
        "t{:>4} stored {:>8.3}  free {:>7.3}  pore {:>7.3}  aquifer {:>7.3} (head {:>5.2} m)  \
         rain {:>7.3}  user {:>6.3}  evap {:>6.3}  outlet {:>6.3}  residual {:>10.2e}",
        v.tick,
        v.stored_m3(),
        free,
        pore,
        v.aquifer_m3,
        world.aquifer_head_m(),
        l.rain_in,
        l.user_in,
        l.evaporation_out,
        l.outlet_out,
        v.stored_m3() - l.expected_stored(),
    );
    if let (Some(s), Some(o)) = (world.spring_cell(), world.outlet_cell()) {
        println!(
            "        spring ({},{},{}) fill {:.3}   outlet ({},{},{}) fill {:.3} {}",
            s.0,
            s.1,
            s.2,
            v.free_at(s.0 as i64, s.1, s.2),
            o.0,
            o.1,
            o.2,
            v.free_at(o.0 as i64, o.1, o.2),
            if world.outlet_open() { "open" } else { "closed" }
        );
    }
}

fn stores(world: &World) -> (f64, f64) {
    let v = world.view();
    let vol = v.config.voxel_volume();
    let mut free = 0.0;
    let mut pore = 0.0;
    for (i, m) in v.material.iter().enumerate() {
        if !m.is_solid() {
            free += v.free[i] as f64 * vol;
        }
        pore += v.pore[i] as f64 * vol * m.pore_capacity();
    }
    (free, pore)
}

/// Side-on: for every column of pixels, the nearest voxel that stops the eye. Water is
/// drawn only up to its fill height, so a surface line reads.
fn write_png(world: &World, path: &str) -> anyhow::Result<()> {
    let v = world.view();
    let c = v.config;
    let (w, h) = (c.width as usize * SCALE, c.height as usize * SCALE);
    let sky = [16u8, 18, 28];
    let mut buf = vec![0u8; w * h * 3];

    for py in 0..h {
        for px in 0..w {
            let x = (px / SCALE) as i64;
            let y_from_top = py / SCALE;
            let y = (c.height as usize - 1 - y_from_top) as u32;
            // Where inside the voxel this pixel row sits, 0 at the floor.
            let sub = (SCALE - 1 - (py % SCALE)) as f64 / SCALE as f64;

            let mut rgb = sky;
            for z in 0..c.depth {
                let m = v.material_at(x, y, z);
                let shade = 1.0 - 0.45 * (z as f64 / (c.depth.max(2) - 1) as f64);
                if m.is_solid() {
                    rgb = tint(material_rgb(m, v.pore_at(x, y, z)), shade);
                    break;
                }
                let fill = v.free_at(x, y, z) as f64;
                if fill > 0.02 && sub < fill {
                    let top = fill - sub < 1.0 / SCALE as f64;
                    let water = if top { [120u8, 196, 236] } else { [46, 104, 176] };
                    rgb = tint(water, shade);
                    break;
                }
            }
            let o = (py * w + px) * 3;
            buf[o..o + 3].copy_from_slice(&rgb);
        }
    }

    let file = BufWriter::new(File::create(path)?);
    let mut encoder = png::Encoder::new(file, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&buf)?;
    Ok(())
}

fn material_rgb(m: Material, pore: f32) -> [u8; 3] {
    let base = match m {
        Material::Air => [16, 18, 28],
        Material::Bedrock => [48, 52, 64],
        Material::Rock => [98, 100, 112],
        Material::Soil => [116, 86, 58],
    };
    // Wet ground reads darker and cooler.
    let wet = 1.0 - 0.28 * pore as f64;
    [
        (base[0] as f64 * wet) as u8,
        (base[1] as f64 * wet) as u8,
        (base[2] as f64 * wet).min(255.0) as u8,
    ]
}

fn tint(rgb: [u8; 3], shade: f64) -> [u8; 3] {
    let sky = [16.0, 18.0, 28.0];
    let mut out = [0u8; 3];
    for k in 0..3 {
        let lit = rgb[k] as f64 * shade;
        // Atmospheric fade toward the back wall.
        out[k] = (lit + (1.0 - shade) * sky[k] * 0.9).clamp(0.0, 255.0) as u8;
    }
    out
}
