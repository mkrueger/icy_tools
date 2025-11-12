#[cfg(test)]
mod test {
    use icy_engine::igs::vdi::{color_idx_to_pixel_val, pixel_val_to_color_idx};

    #[test]
    pub fn test_pixel_conversation() {
        for i in 0..16u8 {
            assert_eq!(i, color_idx_to_pixel_val(16, pixel_val_to_color_idx(16, i)));
            assert_eq!(i, pixel_val_to_color_idx(16, color_idx_to_pixel_val(16, i)));
        }
    }
}

#[cfg(test)]
mod test_loop_bug2 {
    use icy_engine::{
        Position,
        igs::vdi::{calculate_point, icos, isin},
    };
    use pretty_assertions::assert_eq;

    #[test]
    pub fn test_isin() {
        let vec = (0..90).step_by(5).map(|i| isin(i * 10)).collect::<Vec<_>>();
        let expected = vec![
            0,     // isin(0)
            5711,  // isin(50)
            11380, // isin(100)
            16961, // isin(150)
            22415, // isin(200)
            27696, // isin(250)
            32767, // isin(300)
            37589, // isin(350)
            42126, // isin(400)
            46340, // isin(450)
            50202, // isin(500)
            53683, // isin(550)
            56756, // isin(600)
            59395, // isin(650)
            61582, // isin(700)
            63302, // isin(750)
            64540, // isin(800)
            65285, // isin(850)
        ];
        assert_eq!(vec, expected);
    }

    #[test]
    pub fn test_isin2() {
        assert_eq!(0, isin(0));
        //assert_eq!(56216, isin(898));
        //assert_eq!(51558, isin(899));
        //assert_eq!(46899, isin(900));
    }

    #[test]
    pub fn test_isin_interpolation() {
        let vec = (0..90).step_by(5).map(|i| isin(i * 10 + 20)).collect::<Vec<_>>();
        let expected = vec![
            2287,  // sin(20)
            7986,  // sin(70)
            13626, // sin(120)
            19160, // sin(170)
            24549, // sin(220)
            29752, // sin(270)
            34729, // sin(320)
            39440, // sin(370)
            43851, // sin(420)
            47929, // sin(470)
            51643, // sin(520)
            54962, // sin(570)
            57863, // sin(620)
            60325, // sin(670)
            62328, // sin(720)
            63854, // sin(770)
            64896, // sin(820)
            65445, // sin(870)
        ];
        assert_eq!(vec, expected);
    }
    #[test]
    pub fn test_icos() {
        let vec = (5..90).step_by(5).map(|i| icos(i * 10)).collect::<Vec<_>>();
        let expected = vec![
            65285, // icos(50)
            64540, // icos(100)
            63302, // icos(150)
            61582, // icos(200)
            59395, // icos(250)
            56756, // icos(300)
            53683, // icos(350)
            50202, // icos(400)
            46340, // icos(450)
            42126, // icos(500)
            37589, // icos(550)
            32767, // icos(600)
            27696, // icos(650)
            22415, // icos(700)
            16961, // icos(750)
            11380, // icos(800)
            5711,  // icos(850)
        ];
        assert_eq!(vec, expected);
    }

    #[test]
    pub fn test_calc_point() {
        let vec = (0..=36).map(|i| calculate_point(100, 100, 50, 50, i * 100)).collect::<Vec<_>>();

        let expected = vec![
            Position::new(150, 100),
            Position::new(149, 91),
            Position::new(147, 83),
            Position::new(143, 75),
            Position::new(138, 68),
            Position::new(132, 62),
            Position::new(125, 57),
            Position::new(117, 53),
            Position::new(109, 51),
            Position::new(100, 50),
            Position::new(91, 51),
            Position::new(83, 53),
            Position::new(75, 57),
            Position::new(68, 62),
            Position::new(62, 68),
            Position::new(57, 75),
            Position::new(53, 83),
            Position::new(51, 91),
            Position::new(50, 100),
            Position::new(51, 109),
            Position::new(53, 117),
            Position::new(57, 125),
            Position::new(62, 132),
            Position::new(68, 138),
            Position::new(75, 143),
            Position::new(83, 147),
            Position::new(91, 149),
            Position::new(100, 150),
            Position::new(109, 149),
            Position::new(117, 147),
            Position::new(125, 143),
            Position::new(132, 138),
            Position::new(138, 132),
            Position::new(143, 125),
            Position::new(147, 117),
            Position::new(149, 109),
            Position::new(150, 100),
        ];
        assert_eq!(vec, expected);
    }
}
