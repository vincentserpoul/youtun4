use image::{Rgba, RgbaImage};
use std::path::Path;

const SIZE: u32 = 1024;

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn lerp_color(a: [u8; 4], b: [u8; 4], t: f64) -> [u8; 4] {
    [
        lerp(a[0] as f64, b[0] as f64, t) as u8,
        lerp(a[1] as f64, b[1] as f64, t) as u8,
        lerp(a[2] as f64, b[2] as f64, t) as u8,
        lerp(a[3] as f64, b[3] as f64, t) as u8,
    ]
}

fn blend(base: [u8; 4], over: [u8; 4]) -> [u8; 4] {
    let a = over[3] as f64 / 255.0;
    [
        (base[0] as f64 * (1.0 - a) + over[0] as f64 * a) as u8,
        (base[1] as f64 * (1.0 - a) + over[1] as f64 * a) as u8,
        (base[2] as f64 * (1.0 - a) + over[2] as f64 * a) as u8,
        255,
    ]
}

fn dist(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt()
}

fn rounded_rect_mask(x: f64, y: f64, w: f64, h: f64, r: f64, px: f64, py: f64) -> f64 {
    // Returns 0.0 (outside) to 1.0 (inside) with antialiased edges
    let inner_x = px.clamp(x + r, x + w - r);
    let inner_y = py.clamp(y + r, y + h - r);
    let d = dist(px, py, inner_x, inner_y);
    if px >= x + r && px <= x + w - r && py >= y + r && py <= y + h - r {
        1.0
    } else {
        (1.0 - (d - r + 0.5)).clamp(0.0, 1.0)
    }
}

fn main() {
    let s = SIZE as f64;
    let mut img = RgbaImage::new(SIZE, SIZE);

    let bg = [10, 4, 24, 255u8]; // dark purple
    let sun_top = [253, 180, 40, 255u8]; // orange
    let sun_bot = [246, 114, 202, 255u8]; // pink

    let sun_cx = s * 0.5;
    let sun_cy = s * 0.42;
    let sun_r = s * 0.22;
    let corner_r = s * 0.18;

    // Band positions (as fraction of sun diameter, from bottom)
    let bands: Vec<(f64, f64)> = vec![
        (0.05, 0.08), // thin gap at bottom
        (0.15, 0.05),
        (0.24, 0.045),
        (0.33, 0.04),
        (0.42, 0.035),
    ];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = x as f64;
            let py = (SIZE - 1 - y) as f64; // flip Y for math coords

            // Rounded rect mask
            let mask = rounded_rect_mask(0.0, 0.0, s, s, corner_r, px, py);
            if mask <= 0.0 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                continue;
            }

            let mut color = bg;

            // Sun circle
            let d = dist(px, py, sun_cx, sun_cy);
            if d < sun_r + 1.0 {
                let sun_mask = (1.0 - (d - sun_r + 0.5)).clamp(0.0, 1.0);
                let t = ((sun_cy + sun_r - py) / (sun_r * 2.0)).clamp(0.0, 1.0);
                let mut sun_color = lerp_color(sun_top, sun_bot, t);

                // Draw dark bands in bottom half of sun
                let frac = (sun_cy + sun_r - py) / (sun_r * 2.0); // 0=top, 1=bottom
                for &(start, height) in &bands {
                    if frac >= start && frac < start + height {
                        sun_color = bg;
                        break;
                    }
                }

                sun_color[3] = (sun_mask * 255.0) as u8;
                color = blend(color, sun_color);
            }

            // Sun glow
            if d < sun_r * 2.5 && d > sun_r {
                let glow_t = ((d - sun_r) / (sun_r * 1.5)).clamp(0.0, 1.0);
                let glow_a = (1.0 - glow_t).powi(2) * 0.35;
                let glow_color = [246, 114, 202, (glow_a * 255.0) as u8];
                color = blend(color, glow_color);
            }

            // Neon "Y" tuning fork
            let y_stem_top = s * 0.58;
            let y_stem_bot = s * 0.28;
            let y_fork = s * 0.58;
            let y_tip = s * 0.75;
            let spread = s * 0.14;
            let stroke = s * 0.022;
            let glow_r = s * 0.06;

            // Stem distance
            let stem_d = if py >= y_stem_bot && py <= y_stem_top {
                (px - sun_cx).abs()
            } else {
                f64::MAX
            };

            // Left arm: from (cx, y_fork) to (cx - spread, y_tip)
            let left_d = if py >= y_fork && py <= y_tip {
                let t_arm = (py - y_fork) / (y_tip - y_fork);
                let arm_x = lerp(sun_cx, sun_cx - spread, t_arm);
                (px - arm_x).abs()
            } else {
                f64::MAX
            };

            // Right arm
            let right_d = if py >= y_fork && py <= y_tip {
                let t_arm = (py - y_fork) / (y_tip - y_fork);
                let arm_x = lerp(sun_cx, sun_cx + spread, t_arm);
                (px - arm_x).abs()
            } else {
                f64::MAX
            };

            // Joint (rounded)
            let joint_d = dist(px, py, sun_cx, y_fork);

            let min_d = stem_d.min(left_d).min(right_d).min(joint_d - stroke * 0.5);

            // Glow
            if min_d < glow_r {
                let glow_t = (min_d / glow_r).clamp(0.0, 1.0);
                let ga = (1.0 - glow_t).powi(3) * 0.6;
                let glow = [255, 45, 138, (ga * 255.0) as u8];
                color = blend(color, glow);
            }

            // Core stroke
            if min_d < stroke + 0.5 {
                let edge = (1.0 - (min_d - stroke + 0.5)).clamp(0.0, 1.0);
                let core = [255, 200, 230, (edge * 255.0) as u8];
                color = blend(color, core);
            }

            // Apply rounded rect mask
            color[3] = (mask * 255.0) as u8;
            img.put_pixel(x, y, Rgba(color));
        }
    }

    // Save base icon
    let out_dir = Path::new("src-tauri/icons");

    img.save(out_dir.join("icon.png")).expect("save icon.png");
    println!("Saved icon.png (1024x1024)");

    // Generate all required sizes
    let sizes: Vec<(&str, u32)> = vec![
        ("32x32.png", 32),
        ("64x64.png", 64),
        ("128x128.png", 128),
        ("128x128@2x.png", 256),
        ("Square30x30Logo.png", 30),
        ("Square44x44Logo.png", 44),
        ("Square71x71Logo.png", 71),
        ("Square89x89Logo.png", 89),
        ("Square107x107Logo.png", 107),
        ("Square142x142Logo.png", 142),
        ("Square150x150Logo.png", 150),
        ("Square284x284Logo.png", 284),
        ("Square310x310Logo.png", 310),
        ("StoreLogo.png", 50),
    ];

    for (name, size) in &sizes {
        let resized = image::imageops::resize(&img, *size, *size, image::imageops::FilterType::Lanczos3);
        resized.save(out_dir.join(name)).unwrap_or_else(|e| panic!("save {name}: {e}"));
        println!("Saved {name} ({size}x{size})");
    }

    // iOS icons
    let ios_sizes: Vec<(&str, u32)> = vec![
        ("AppIcon-20x20@1x.png", 20),
        ("AppIcon-20x20@2x.png", 40),
        ("AppIcon-20x20@2x-1.png", 40),
        ("AppIcon-20x20@3x.png", 60),
        ("AppIcon-29x29@1x.png", 29),
        ("AppIcon-29x29@2x.png", 58),
        ("AppIcon-29x29@2x-1.png", 58),
        ("AppIcon-29x29@3x.png", 87),
        ("AppIcon-40x40@1x.png", 40),
        ("AppIcon-40x40@2x.png", 80),
        ("AppIcon-40x40@2x-1.png", 80),
        ("AppIcon-40x40@3x.png", 120),
        ("AppIcon-60x60@2x.png", 120),
        ("AppIcon-60x60@3x.png", 180),
        ("AppIcon-76x76@1x.png", 76),
        ("AppIcon-76x76@2x.png", 152),
        ("AppIcon-83.5x83.5@2x.png", 167),
        ("AppIcon-512@2x.png", 1024),
    ];

    for (name, size) in &ios_sizes {
        let resized = image::imageops::resize(&img, *size, *size, image::imageops::FilterType::Lanczos3);
        resized
            .save(out_dir.join("ios").join(name))
            .unwrap_or_else(|e| panic!("save ios/{name}: {e}"));
        println!("Saved ios/{name} ({size}x{size})");
    }

    // Android icons
    let android_sizes: Vec<(&str, u32)> = vec![
        ("mipmap-mdpi", 48),
        ("mipmap-hdpi", 72),
        ("mipmap-xhdpi", 96),
        ("mipmap-xxhdpi", 144),
        ("mipmap-xxxhdpi", 192),
    ];

    for (dir, size) in &android_sizes {
        let resized = image::imageops::resize(&img, *size, *size, image::imageops::FilterType::Lanczos3);
        let android_dir = out_dir.join("android").join(dir);
        for name in ["ic_launcher.png", "ic_launcher_round.png", "ic_launcher_foreground.png"] {
            resized
                .save(android_dir.join(name))
                .unwrap_or_else(|e| panic!("save android/{dir}/{name}: {e}"));
        }
        println!("Saved android/{dir}/* ({size}x{size})");
    }

    // Generate .icns using iconutil (macOS)
    let iconset = out_dir.join("icon.iconset");
    std::fs::create_dir_all(&iconset).ok();

    let icns_sizes: Vec<(&str, u32)> = vec![
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ];

    for (name, size) in &icns_sizes {
        let resized = image::imageops::resize(&img, *size, *size, image::imageops::FilterType::Lanczos3);
        resized.save(iconset.join(name)).unwrap_or_else(|e| panic!("save {name}: {e}"));
    }

    let status = std::process::Command::new("iconutil")
        .args(["-c", "icns", iconset.to_str().unwrap(), "-o", out_dir.join("icon.icns").to_str().unwrap()])
        .status();

    match status {
        Ok(s) if s.success() => println!("Saved icon.icns"),
        _ => eprintln!("Warning: iconutil failed (macOS only)"),
    }

    std::fs::remove_dir_all(&iconset).ok();

    println!("Done!");
}
