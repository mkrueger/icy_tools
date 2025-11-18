use crate::Position;

pub const PI: u16 = 1800;
pub const HALFPI: u16 = PI / 2;
pub const TWOPI: u16 = PI * 2;

const MAX_TABLE_ANGLE: u16 = 896;

const SINUS_TABLE: [u16; 113] = [
    0, 915, 1830, 2744, 3658, 4572, 5484, 6395, 7305, 8214, 9121, 10026, 10929, 11831, 12729, 13626, 14519, 15410, 16298, 17183, 18064, 18942, 19816, 20686,
    21553, 22415, 23272, 24125, 24974, 25817, 26656, 27489, 28317, 29140, 29956, 30767, 31572, 32371, 33163, 33949, 34729, 35501, 36267, 37026, 37777, 38521,
    39258, 39986, 40708, 41421, 42126, 42823, 43511, 44191, 44862, 45525, 46179, 46824, 47459, 48086, 48703, 49310, 49908, 50496, 51075, 51643, 52201, 52750,
    53287, 53815, 54332, 54838, 55334, 55819, 56293, 56756, 57208, 57649, 58078, 58497, 58903, 59299, 59683, 60055, 60415, 60764, 61101, 61426, 61739, 62040,
    62328, 62605, 62870, 63122, 63362, 63589, 63804, 64007, 64197, 64375, 64540, 64693, 64833, 64960, 65075, 65177, 65266, 65343, 65407, 65458, 65496, 65522,
    65534,
];

pub fn isin(angle: u16) -> u16 {
    let angle = angle % MAX_TABLE_ANGLE;
    let index = (angle >> 3) as u16;
    let remainder = (angle & 7) as u16;
    let tmpsin = SINUS_TABLE[index as usize];
    if remainder != 0 && index < SINUS_TABLE.len() as u16 - 1 {
        let next_sin = SINUS_TABLE[index as usize + 1] as i32;
        let curr_sin = tmpsin as i32;
        return (curr_sin + (((next_sin - curr_sin) * remainder as i32) >> 3)) as u16;
    }
    tmpsin
}

pub fn icos(angle: u16) -> u16 {
    isin(HALFPI.wrapping_sub(angle))
}

pub fn calculate_point(xc: i32, yc: i32, x_rad: i32, y_rad: i32, angle: u16) -> Position {
    let mut delta_x;
    let mut delta_y;

    let mut angle = angle % TWOPI;

    let mut negative = 1;
    if angle > 3 * HALFPI {
        angle = TWOPI - angle;
        negative = 0;
    } else if angle > PI {
        angle -= PI;
        negative = 2;
    } else if angle > HALFPI {
        angle = PI - angle;
        negative = 3;
    }
    if angle > MAX_TABLE_ANGLE {
        delta_x = 0;
        delta_y = y_rad as i16;
    } else if angle < HALFPI - MAX_TABLE_ANGLE {
        delta_x = x_rad as i16;
        delta_y = 0;
    } else {
        delta_x = umul_shift(icos(angle), x_rad) as i16;
        delta_y = umul_shift(isin(angle), y_rad) as i16;
    }
    if negative & 2 != 0 {
        delta_x = -delta_x;
    }
    if negative & 1 != 0 {
        delta_y = -delta_y;
    }
    Position::new(xc + delta_x as i32, yc + delta_y as i32)
}

fn umul_shift(a: u16, b: i32) -> u16 {
    let a = a as i64;
    let b = b as i64;
    ((a * b + 32768) >> 16) as u16
}

const MIN_ARC_CT: i32 = 32;
const MAX_ARC_CT: i32 = 128;

fn clc_nsteps(x_rad: i32, y_rad: i32) -> i32 {
    (x_rad.max(y_rad) >> 2).clamp(MIN_ARC_CT, MAX_ARC_CT)
}

pub fn gdp_curve(xm: i32, ym: i32, x_rad: i32, y_rad: i32, beg_ang: i32, end_ang: i32) -> Vec<i32> {
    let mut del_ang = end_ang - beg_ang;
    if del_ang < 0 {
        del_ang += TWOPI as i32;
    }
    let x_rad: i32 = x_rad.abs();
    let y_rad = y_rad.abs();
    let steps = clc_nsteps(x_rad, y_rad);
    clc_arc(xm, ym, x_rad, y_rad, beg_ang, end_ang, del_ang, steps)
}

pub fn clc_arc(xm: i32, ym: i32, x_rad: i32, y_rad: i32, beg_ang: i32, end_ang: i32, del_ang: i32, steps: i32) -> Vec<i32> {
    let mut points = Vec::new();
    let start = beg_ang;
    let p = calculate_point(xm, ym, x_rad, y_rad, beg_ang as u16);
    let mut last_p = p;
    points.push(p.x);
    points.push(p.y);
    for i in 1..steps {
        let angle = del_ang * i / steps + start;
        let p = calculate_point(xm, ym, x_rad, y_rad, angle as u16);
        if last_p != p {
            points.push(p.x);
            points.push(p.y);
            last_p = p;
        }
    }
    let p = calculate_point(xm, ym, x_rad, y_rad, end_ang as u16);
    points.push(p.x);
    points.push(p.y);
    points
}

pub fn color_idx_to_pixel_val(_colors: usize, c: u8) -> u8 {
    return c; /* 
    const COLOR_TO_PIX_TABLE: [u8; 16] = [0, 15, 1, 2, 4, 6, 3, 5, 7, 8, 9, 10, 12, 14, 11, 13];

    if colors == 16 {
    return COLOR_TO_PIX_TABLE[c as usize];
    }

    if colors == 4 {
    return match c {
    0 => 0,
    1 => 3,
    2 => 1,
    3 => 2,
    _ => c,
    };
    }
    return c;*/
}

pub fn pixel_val_to_color_idx(_colors: usize, c: u8) -> u8 {
    return c; /* 
    const PIX_TO_COLOR_TABLE: [u8; 16] = [0, 2, 3, 6, 4, 7, 5, 8, 9, 10, 11, 14, 12, 15, 13, 1];
    if colors == 16 {
    return PIX_TO_COLOR_TABLE[c as usize];
    }
    // THIS IS A GUESS. THE REFERENCE BOOKS ONLY GIVE TABLES FOR 8-bit and 16-bit PALETTES.
    // NEED TO DOUBLE-CHECK ON REAL ATARI.
    if colors == 4 {
    return match c {
    0 => 0,
    1 => 2,
    2 => 3,
    3 => 1,
    _ => c,
    };
    }
    return c;*/
}

pub fn blit_px(write_mode: i32, colors: usize, s: u8, d: u8) -> u8 {
    let s = color_idx_to_pixel_val(colors, s);
    let d = color_idx_to_pixel_val(colors, d);
    let dest = match write_mode {
        0 => 0,
        1 => s & d,
        2 => s & !d,
        3 => s,
        4 => !s & d,
        5 => d,
        6 => s ^ d,
        7 => s | d,
        8 => !(s | d),
        9 => !(s ^ d),
        10 => !d,
        11 => s | !d,
        12 => !s,
        13 => !s | d,
        14 => !(s & d),
        15 => 1,
        _ => 2,
    } & 0xF;
    pixel_val_to_color_idx(colors, dest)
}
