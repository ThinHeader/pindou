//! 色号稿：缩小到珠板尺寸，再量化到拼豆色板。
//!
//! 这是简化像素化，不是印刷级分色。失败时返回可直接展示的中文原因。

use image::{imageops::FilterType, DynamicImage, ImageBuffer, ImageFormat, Rgba, RgbaImage};

use crate::error::AppError;

const WARM: u8 = 1;
const MACARON: u8 = 2;
const CONTRAST: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pref {
    Auto,
    Warm,
    Macaron,
    Contrast,
}

impl Pref {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "" | "auto" => Some(Self::Auto),
            "warm" => Some(Self::Warm),
            "macaron" => Some(Self::Macaron),
            "contrast" => Some(Self::Contrast),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Warm => "warm",
            Self::Macaron => "macaron",
            Self::Contrast => "contrast",
        }
    }
}

struct Bead {
    code: &'static str,
    name: &'static str,
    rgb: [u8; 3],
    tags: u8,
}

/// 常用拼豆色（含设计稿里的 A1 / B3 / C2 / D1 / K9 等色号）。
const BEADS: &[Bead] = &[
    Bead {
        code: "A1",
        name: "珊瑚",
        rgb: [0xFF, 0x6B, 0x5B],
        tags: WARM | CONTRAST,
    },
    Bead {
        code: "B3",
        name: "杏桃",
        rgb: [0xFF, 0xB0, 0x88],
        tags: WARM | MACARON,
    },
    Bead {
        code: "C2",
        name: "粉雾",
        rgb: [0xFF, 0xE8, 0xE4],
        tags: WARM | MACARON,
    },
    Bead {
        code: "D1",
        name: "奶黄",
        rgb: [0xFF, 0xD1, 0x66],
        tags: WARM | MACARON | CONTRAST,
    },
    Bead {
        code: "K9",
        name: "墨黑",
        rgb: [0x2C, 0x2A, 0x29],
        tags: CONTRAST,
    },
    Bead {
        code: "E4",
        name: "浅咖",
        rgb: [0xC9, 0xA8, 0x8A],
        tags: WARM | MACARON,
    },
    Bead {
        code: "E7",
        name: "焦糖",
        rgb: [0xA6, 0x7C, 0x52],
        tags: WARM | CONTRAST,
    },
    Bead {
        code: "W0",
        name: "白米",
        rgb: [0xFF, 0xFF, 0xFF],
        tags: MACARON,
    },
    Bead {
        code: "S1",
        name: "晴空",
        rgb: [0x7B, 0xDF, 0xF2],
        tags: MACARON | CONTRAST,
    },
    Bead {
        code: "G1",
        name: "叶绿",
        rgb: [0x4C, 0xAF, 0x7A],
        tags: CONTRAST,
    },
    Bead {
        code: "M1",
        name: "薄荷",
        rgb: [0xA8, 0xE6, 0xCF],
        tags: MACARON,
    },
    Bead {
        code: "P1",
        name: "丁香",
        rgb: [0xC9, 0xB1, 0xFF],
        tags: MACARON,
    },
    Bead {
        code: "N1",
        name: "奶油",
        rgb: [0xED, 0xE7, 0xE2],
        tags: MACARON,
    },
    Bead {
        code: "H1",
        name: "蜜桃",
        rgb: [0xFF, 0xB4, 0xA2],
        tags: WARM | MACARON,
    },
    Bead {
        code: "R1",
        name: "正红",
        rgb: [0xE2, 0x3D, 0x3D],
        tags: WARM | CONTRAST,
    },
    Bead {
        code: "R2",
        name: "玫红",
        rgb: [0xE8, 0x5A, 0x71],
        tags: WARM | CONTRAST,
    },
    Bead {
        code: "O1",
        name: "柿橙",
        rgb: [0xFF, 0x8C, 0x42],
        tags: WARM | CONTRAST,
    },
    Bead {
        code: "Y1",
        name: "柠檬黄",
        rgb: [0xF5, 0xD9, 0x0A],
        tags: WARM | CONTRAST,
    },
    Bead {
        code: "Y2",
        name: "杏黄",
        rgb: [0xF6, 0xC1, 0x77],
        tags: WARM | MACARON,
    },
    Bead {
        code: "G2",
        name: "豆绿",
        rgb: [0x8F, 0xD1, 0x8A],
        tags: MACARON,
    },
    Bead {
        code: "G3",
        name: "松绿",
        rgb: [0x2F, 0x6D, 0x4F],
        tags: CONTRAST,
    },
    Bead {
        code: "B1",
        name: "海蓝",
        rgb: [0x3A, 0x7C, 0xA5],
        tags: CONTRAST,
    },
    Bead {
        code: "B2",
        name: "浅蓝",
        rgb: [0xB7, 0xDF, 0xF7],
        tags: MACARON,
    },
    Bead {
        code: "P2",
        name: "葡萄",
        rgb: [0x7A, 0x5A, 0xF8],
        tags: CONTRAST,
    },
    Bead {
        code: "N2",
        name: "暖灰",
        rgb: [0xB7, 0xA9, 0x9A],
        tags: WARM,
    },
    Bead {
        code: "N3",
        name: "炭灰",
        rgb: [0x5C, 0x58, 0x56],
        tags: CONTRAST,
    },
    Bead {
        code: "K1",
        name: "深棕",
        rgb: [0x6B, 0x3E, 0x26],
        tags: WARM | CONTRAST,
    },
    Bead {
        code: "F1",
        name: "肤色",
        rgb: [0xF3, 0xC6, 0xA8],
        tags: WARM | MACARON,
    },
    Bead {
        code: "C1",
        name: "樱粉",
        rgb: [0xFF, 0x8F, 0xAB],
        tags: WARM | MACARON,
    },
    Bead {
        code: "T1",
        name: "青绿",
        rgb: [0x2B, 0xBB, 0xAD],
        tags: CONTRAST,
    },
    Bead {
        code: "L1",
        name: "淡紫",
        rgb: [0xE6, 0xD9, 0xFF],
        tags: MACARON,
    },
    Bead {
        code: "W1",
        name: "米白",
        rgb: [0xF7, 0xF1, 0xEA],
        tags: WARM | MACARON,
    },
];

#[derive(Clone, Debug)]
pub struct Swatch {
    pub code: String,
    pub name: String,
    pub hex: String,
    pub rgb: [u8; 3],
    pub count: i32,
}

#[derive(Clone, Debug)]
pub struct Pattern {
    pub width: u32,
    pub palette: Vec<Swatch>,
    pub cells: Vec<u8>,
}

#[derive(Debug)]
pub enum PixelError {
    BadBoard,
    BadColorCount,
    TooLarge,
    Decode,
    Empty,
    UnknownScene,
}

impl PixelError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::BadBoard => "珠板尺寸仅支持 52、78、104",
            Self::BadColorCount => "色数需在 4 到 24 之间",
            Self::TooLarge => "图片过大，请压缩到 6MB 以内再试",
            Self::Decode => "无法解析图片，请上传 JPG 或 PNG",
            Self::Empty => "图片是空的，请换一张再试",
            Self::UnknownScene => "没有这张示例图",
        }
    }
}

impl From<PixelError> for AppError {
    fn from(err: PixelError) -> Self {
        AppError::BadRequest(err.message().into())
    }
}

pub fn parse_board(size: Option<u32>, label: Option<&str>) -> Result<u32, PixelError> {
    if let Some(n) = size {
        return check_board(n);
    }
    if let Some(label) = label.map(str::trim).filter(|s| !s.is_empty()) {
        let head = label.split(['x', 'X', '×']).next().unwrap_or("").trim();
        let n = head.parse::<u32>().unwrap_or(0);
        return check_board(n);
    }
    Ok(52)
}

fn check_board(n: u32) -> Result<u32, PixelError> {
    if matches!(n, 52 | 78 | 104) {
        Ok(n)
    } else {
        Err(PixelError::BadBoard)
    }
}

pub fn parse_color_count(n: Option<u32>) -> Result<u32, PixelError> {
    let n = n.unwrap_or(12);
    if (4..=24).contains(&n) {
        Ok(n)
    } else {
        Err(PixelError::BadColorCount)
    }
}

pub fn pixelate(
    bytes: &[u8],
    board: u32,
    color_count: u32,
    pref: Pref,
) -> Result<Pattern, PixelError> {
    check_board(board)?;
    parse_color_count(Some(color_count))?;
    if bytes.len() > 6 * 1024 * 1024 {
        return Err(PixelError::TooLarge);
    }
    let img = image::load_from_memory(bytes).map_err(|_| PixelError::Decode)?;
    if img.width() == 0 || img.height() == 0 {
        return Err(PixelError::Empty);
    }
    if img.width() as u64 * img.height() as u64 > 20_000_000 {
        return Err(PixelError::TooLarge);
    }
    let flat = flatten_on_white(&img);
    let small = image::imageops::resize(&flat, board, board, FilterType::Triangle);
    let mut assign = Vec::with_capacity((board * board) as usize);
    for pixel in small.pixels() {
        assign.push(nearest([pixel[0], pixel[1], pixel[2]], pref));
    }
    reduce_colors(&mut assign, color_count as usize);
    Ok(compact(board, &assign))
}

pub fn render_preview_png(pattern: &Pattern) -> Result<Vec<u8>, PixelError> {
    let n = pattern.width as i32;
    let cell: i32 = if pattern.width <= 52 {
        12
    } else if pattern.width <= 78 {
        8
    } else {
        6
    };
    let pad = 16i32;
    let board = n * cell;
    let side = (board + pad * 2) as u32;
    let mut img = RgbaImage::from_pixel(side, side, Rgba([0xF7, 0xF3, 0xF0, 255]));
    fill_rect(
        &mut img,
        pad - 6,
        pad - 6,
        board + 12,
        board + 12,
        Rgba([0xE4, 0xDB, 0xD4, 255]),
    );

    for y in 0..n {
        for x in 0..n {
            let idx = pattern.cells[(y * n + x) as usize] as usize;
            let rgb = pattern.palette.get(idx).map(|s| s.rgb).unwrap_or([0, 0, 0]);
            let cx = pad + x * cell + cell / 2;
            let cy = pad + y * cell + cell / 2;
            let hole = (cell as f32 * 0.16).max(0.8);
            fill_circle(&mut img, cx, cy, hole, Rgba([0, 0, 0, 28]));
            let rad = cell as f32 * 0.36;
            fill_circle(&mut img, cx, cy, rad, Rgba([rgb[0], rgb[1], rgb[2], 255]));
            if cell >= 8 {
                fill_circle(
                    &mut img,
                    cx - (rad * 0.28) as i32,
                    cy - (rad * 0.32) as i32,
                    rad * 0.28,
                    Rgba([255, 255, 255, 90]),
                );
            }
        }
    }
    encode_png(&img)
}

pub fn scene_png(name: &str) -> Result<Vec<u8>, PixelError> {
    let img = render_scene(name)?;
    encode_png(&img)
}

fn render_scene(name: &str) -> Result<RgbaImage, PixelError> {
    let n = 180i32;
    let bg = match name {
        "cat" => [0xFF, 0xF5, 0xF0],
        "bear" => [0xFF, 0xFB, 0xEA],
        "chick" => [0xF0, 0xFB, 0xFF],
        "bunny" => [0xFF, 0xF0, 0xF3],
        _ => return Err(PixelError::UnknownScene),
    };
    let mut img = RgbaImage::from_pixel(n as u32, n as u32, Rgba([bg[0], bg[1], bg[2], 255]));
    match name {
        "cat" => {
            fill_ellipse(&mut img, 32, 28, 22, 26, Rgba([0xFF, 0x6B, 0x5B, 255]));
            fill_ellipse(&mut img, 148, 28, 22, 26, Rgba([0xFF, 0x6B, 0x5B, 255]));
            fill_ellipse(&mut img, 90, 78, 62, 54, Rgba([0xFF, 0xB0, 0x88, 255]));
            fill_ellipse(&mut img, 90, 92, 36, 28, Rgba([0xFF, 0xC9, 0xA8, 255]));
            fill_ellipse(&mut img, 66, 72, 10, 13, Rgba([0x2C, 0x2A, 0x29, 255]));
            fill_ellipse(&mut img, 114, 72, 10, 13, Rgba([0x2C, 0x2A, 0x29, 255]));
            fill_ellipse(&mut img, 70, 68, 3, 3, Rgba([255, 255, 255, 255]));
            fill_ellipse(&mut img, 118, 68, 3, 3, Rgba([255, 255, 255, 255]));
            fill_ellipse(&mut img, 90, 90, 7, 5, Rgba([0xFF, 0x6B, 0x5B, 255]));
            fill_ellipse(&mut img, 90, 104, 12, 9, Rgba([0xFF, 0x6B, 0x5B, 255]));
            fill_ellipse(&mut img, 48, 96, 10, 7, Rgba([0xFF, 0xD1, 0x66, 255]));
            fill_ellipse(&mut img, 132, 96, 10, 7, Rgba([0xFF, 0xD1, 0x66, 255]));
        }
        "bear" => {
            fill_ellipse(&mut img, 40, 36, 22, 22, Rgba([0xFF, 0xD1, 0x66, 255]));
            fill_ellipse(&mut img, 140, 36, 22, 22, Rgba([0xFF, 0xD1, 0x66, 255]));
            fill_ellipse(&mut img, 90, 86, 58, 50, Rgba([0xFF, 0xB0, 0x88, 255]));
            fill_ellipse(&mut img, 64, 78, 9, 11, Rgba([0x2C, 0x2A, 0x29, 255]));
            fill_ellipse(&mut img, 116, 78, 9, 11, Rgba([0x2C, 0x2A, 0x29, 255]));
            fill_ellipse(&mut img, 90, 98, 8, 6, Rgba([0xFF, 0x6B, 0x5B, 255]));
            fill_ellipse(&mut img, 78, 112, 8, 6, Rgba([0xE8, 0x5A, 0x71, 255]));
            fill_ellipse(&mut img, 102, 112, 8, 6, Rgba([0xE8, 0x5A, 0x71, 255]));
        }
        "chick" => {
            fill_ellipse(&mut img, 90, 96, 56, 48, Rgba([0xFF, 0xD1, 0x66, 255]));
            fill_ellipse(&mut img, 90, 58, 36, 32, Rgba([0xF6, 0xC1, 0x77, 255]));
            fill_ellipse(&mut img, 74, 54, 8, 10, Rgba([0x2C, 0x2A, 0x29, 255]));
            fill_ellipse(&mut img, 106, 54, 8, 10, Rgba([0x2C, 0x2A, 0x29, 255]));
            fill_ellipse(&mut img, 90, 72, 10, 7, Rgba([0xFF, 0x6B, 0x5B, 255]));
            fill_ellipse(&mut img, 28, 90, 18, 12, Rgba([0x7B, 0xDF, 0xF2, 255]));
            fill_ellipse(&mut img, 152, 90, 18, 12, Rgba([0x7B, 0xDF, 0xF2, 255]));
        }
        "bunny" => {
            fill_ellipse(&mut img, 62, 36, 14, 32, Rgba([0xFF, 0xB4, 0xA2, 255]));
            fill_ellipse(&mut img, 118, 36, 14, 32, Rgba([0xFF, 0xB4, 0xA2, 255]));
            fill_ellipse(&mut img, 62, 40, 6, 18, Rgba([0xFF, 0x8F, 0xAB, 255]));
            fill_ellipse(&mut img, 118, 40, 6, 18, Rgba([0xFF, 0x8F, 0xAB, 255]));
            fill_ellipse(&mut img, 90, 96, 52, 46, Rgba([0xFF, 0xFF, 0xFF, 255]));
            fill_ellipse(&mut img, 70, 90, 7, 9, Rgba([0x2C, 0x2A, 0x29, 255]));
            fill_ellipse(&mut img, 110, 90, 7, 9, Rgba([0x2C, 0x2A, 0x29, 255]));
            fill_ellipse(&mut img, 90, 108, 6, 5, Rgba([0xFF, 0x6B, 0x5B, 255]));
            fill_ellipse(&mut img, 90, 122, 16, 8, Rgba([0xFF, 0xB4, 0xA2, 255]));
        }
        _ => return Err(PixelError::UnknownScene),
    }
    Ok(img)
}

fn flatten_on_white(img: &DynamicImage) -> RgbaImage {
    let rgba = img.to_rgba8();
    let mut out = ImageBuffer::new(rgba.width(), rgba.height());
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
        out.put_pixel(x, y, Rgba([mix(p[0]), mix(p[1]), mix(p[2]), 255]));
    }
    out
}

fn nearest(rgb: [u8; 3], pref: Pref) -> u8 {
    let mut best_i = 0u8;
    let mut best_d = i32::MAX;
    for (i, bead) in BEADS.iter().enumerate() {
        let d = dist2(rgb, bead.rgb) + penalty(bead, pref);
        if d < best_d {
            best_d = d;
            best_i = i as u8;
        }
    }
    best_i
}

fn penalty(bead: &Bead, pref: Pref) -> i32 {
    let hit = match pref {
        Pref::Auto => true,
        Pref::Warm => bead.tags & WARM != 0,
        Pref::Macaron => bead.tags & MACARON != 0,
        Pref::Contrast => bead.tags & CONTRAST != 0,
    };
    if hit {
        0
    } else {
        7_000
    }
}

fn dist2(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

fn reduce_colors(assign: &mut [u8], limit: usize) {
    if limit == 0 {
        return;
    }
    loop {
        let mut counts = [0usize; 64];
        for &a in assign.iter() {
            counts[a as usize] += 1;
        }
        let used: Vec<u8> = (0..BEADS.len())
            .filter(|&i| counts[i] > 0)
            .map(|i| i as u8)
            .collect();
        if used.len() <= limit {
            break;
        }
        let victim = used
            .iter()
            .copied()
            .min_by_key(|i| counts[*i as usize])
            .unwrap();
        let mut repl = used[0];
        let mut best = i32::MAX;
        for &other in &used {
            if other == victim {
                continue;
            }
            let d = dist2(BEADS[victim as usize].rgb, BEADS[other as usize].rgb);
            if d < best {
                best = d;
                repl = other;
            }
        }
        if repl == victim {
            break;
        }
        for slot in assign.iter_mut() {
            if *slot == victim {
                *slot = repl;
            }
        }
    }
}

fn compact(width: u32, assign: &[u8]) -> Pattern {
    let mut order: Vec<u8> = Vec::new();
    let mut counts = [0i32; 64];
    for &a in assign {
        if counts[a as usize] == 0 {
            order.push(a);
        }
        counts[a as usize] += 1;
    }
    order.sort_by_key(|i| std::cmp::Reverse(counts[*i as usize]));
    let mut remap = [0u8; 64];
    let mut palette = Vec::with_capacity(order.len());
    for (new_i, bead_i) in order.iter().copied().enumerate() {
        remap[bead_i as usize] = new_i as u8;
        let bead = &BEADS[bead_i as usize];
        palette.push(Swatch {
            code: bead.code.to_string(),
            name: bead.name.to_string(),
            hex: format!("#{:02X}{:02X}{:02X}", bead.rgb[0], bead.rgb[1], bead.rgb[2]),
            rgb: bead.rgb,
            count: counts[bead_i as usize],
        });
    }
    let cells = assign.iter().map(|a| remap[*a as usize]).collect();
    Pattern {
        width,
        palette,
        cells,
    }
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, PixelError> {
    let mut buf = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|_| PixelError::Decode)?;
    Ok(buf.into_inner())
}

fn fill_rect(img: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, color: Rgba<u8>) {
    for yy in y..y + h {
        for xx in x..x + w {
            put(img, xx, yy, color);
        }
    }
}

fn fill_circle(img: &mut RgbaImage, cx: i32, cy: i32, rad: f32, color: Rgba<u8>) {
    let r = rad.ceil() as i32 + 1;
    let r2 = rad * rad;
    for dy in -r..=r {
        for dx in -r..=r {
            let d = (dx * dx + dy * dy) as f32;
            if d <= r2 {
                let mut c = color;
                if color[3] < 255 {
                    // 简单覆盖：半透明高光直接写，预览不要求精确 alpha 合成
                    if d > r2 * 0.45 {
                        continue;
                    }
                    c[3] = 255;
                    let base = img.get_pixel(clamp_x(img, cx + dx), clamp_y(img, cy + dy));
                    c[0] = blend(base[0], color[0], color[3]);
                    c[1] = blend(base[1], color[1], color[3]);
                    c[2] = blend(base[2], color[2], color[3]);
                }
                put(img, cx + dx, cy + dy, c);
            }
        }
    }
}

fn blend(dst: u8, src: u8, a: u8) -> u8 {
    let a = a as u32;
    ((src as u32 * a + dst as u32 * (255 - a)) / 255) as u8
}

fn clamp_x(img: &RgbaImage, x: i32) -> u32 {
    x.clamp(0, img.width() as i32 - 1) as u32
}
fn clamp_y(img: &RgbaImage, y: i32) -> u32 {
    y.clamp(0, img.height() as i32 - 1) as u32
}

fn fill_ellipse(img: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, color: Rgba<u8>) {
    if rx <= 0 || ry <= 0 {
        return;
    }
    let rx2 = (rx * rx) as f32;
    let ry2 = (ry * ry) as f32;
    for y in (cy - ry)..=(cy + ry) {
        for x in (cx - rx)..=(cx + rx) {
            let dx = (x - cx) as f32;
            let dy = (y - cy) as f32;
            if dx * dx / rx2 + dy * dy / ry2 <= 1.0 {
                put(img, x, y, color);
            }
        }
    }
}

fn put(img: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        img.put_pixel(x as u32, y as u32, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_png(rgb: [u8; 3]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(8, 8, Rgba([rgb[0], rgb[1], rgb[2], 255]));
        encode_png(&img).unwrap()
    }

    #[test]
    fn rejects_garbage_and_bad_params() {
        let err = pixelate(b"not-an-image", 52, 8, Pref::Auto).unwrap_err();
        assert_eq!(err.message(), "无法解析图片，请上传 JPG 或 PNG");
        assert!(parse_board(Some(29), None).is_err());
        assert!(parse_color_count(Some(3)).is_err());
        assert!(parse_color_count(Some(30)).is_err());
        assert_eq!(parse_board(None, Some("78x78")).unwrap(), 78);
        assert_eq!(parse_board(None, Some("104×104")).unwrap(), 104);
    }

    #[test]
    fn solid_color_collapses_to_one_swatch() {
        let png = solid_png([0xFF, 0x6B, 0x5B]);
        let pattern = pixelate(&png, 52, 8, Pref::Auto).unwrap();
        assert_eq!(pattern.cells.len(), 52 * 52);
        assert_eq!(pattern.palette.len(), 1);
        assert_eq!(pattern.palette[0].code, "A1");
        assert_eq!(pattern.palette[0].count, 52 * 52);
        let preview = render_preview_png(&pattern).unwrap();
        assert!(preview.starts_with(b"\x89PNG"));
    }

    #[test]
    fn scene_respects_color_cap() {
        let png = scene_png("cat").unwrap();
        let pattern = pixelate(&png, 52, 6, Pref::Warm).unwrap();
        assert!(pattern.palette.len() <= 6);
        assert!(pattern.palette.len() >= 2);
        let sum: i32 = pattern.palette.iter().map(|s| s.count).sum();
        assert_eq!(sum, 52 * 52);
        assert!(pattern
            .palette
            .iter()
            .any(|s| s.code == "A1" || s.code == "B3"));
    }
}
