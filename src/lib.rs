use image::GenericImageView;
use image::GrayImage;
use image::ImageBuffer;
use image::Luma;

fn calc_lut_body<T, const HIST_SIZE: usize>(
    lut: &mut [u32; HIST_SIZE],
    src: &ImageBuffer<Luma<T>, Vec<T>>,
    tile_size_wh: (usize, usize),
    clip_limit: i32,
    lut_scale: f32,
    tile_x: usize,
    tile_y: usize,
) where
    T: image::Primitive,
{
    let tile = src.view(
        (tile_x * tile_size_wh.0) as u32,
        (tile_y * tile_size_wh.1) as u32,
        tile_size_wh.0 as u32,
        tile_size_wh.1 as u32,
    );

    let mut tile_hist: [u32; HIST_SIZE] = [0; HIST_SIZE];
    for (_x, _y, v) in tile.pixels() {
        tile_hist[v[0].to_usize().expect("failed to convert T to usize")] += 1;
    }

    // clip histogram
    if clip_limit > 0 {
        let clip_limit = clip_limit as u32;

        let mut clipped: usize = 0;
        for bin in tile_hist.iter_mut() {
            if *bin > clip_limit {
                clipped += (*bin - clip_limit) as usize;
                *bin = clip_limit;
            }
        }

        let redist_batch = clipped / HIST_SIZE;
        let mut residual = clipped - redist_batch * HIST_SIZE;
        for bin in tile_hist.iter_mut() {
            *bin += redist_batch as u32;
        }

        if residual != 0 {
            let residual_step = HIST_SIZE.checked_div(residual).unwrap_or(1).max(1);
            let mut i = 0;
            while i < HIST_SIZE && residual > 0 {
                tile_hist[i] += 1;
                i += residual_step;
                residual -= 1;
            }
        }
    }

    // calc Lut
    let mut sum = 0;
    for i in 0..HIST_SIZE {
        sum += tile_hist[i];
        lut[i] = (sum as f32 * lut_scale).clamp(0.0, HIST_SIZE as f32 - 1.0) as u32;
    }
}

fn interpolate<T, U, const T_MAX: usize, const U_MAX: usize>(
    dst: &mut ImageBuffer<Luma<U>, Vec<U>>,
    input: &ImageBuffer<Luma<T>, Vec<T>>,
    luts: &[[u32; T_MAX]],
    tile_size_wh: (usize, usize),
    n_tiles_wh: (usize, usize),
    tile_xs: (i32, i32),
    tile_ys: (i32, i32),
) where
    T: image::Primitive,
    U: image::Primitive
        + num_traits::cast::ToPrimitive
        + num_traits::cast::FromPrimitive
        + std::fmt::Display,
{
    let out_width = dst.width() as usize;
    let out_height = dst.height() as usize;

    // Calculate range,
    //  for -1, 0 this should be 0..(tile_width/2)
    //  for 0, 1 this should be (tile_width/2 to 3 tile_width / 2)

    let (tile_width, tile_height) = tile_size_wh;
    let x_start: u32 = (tile_xs.0 * tile_width as i32 + tile_width as i32 / 2)
        .clamp(0i32, out_width as i32) as u32;
    let x_end: u32 = (tile_xs.1 * tile_width as i32 + tile_width as i32 / 2)
        .clamp(0i32, out_width as i32) as u32;

    let y_start: u32 = (tile_ys.0 * tile_height as i32 + tile_height as i32 / 2)
        .clamp(0i32, out_height as i32) as u32;
    let y_end: u32 = (tile_ys.1 * tile_height as i32 + tile_height as i32 / 2)
        .clamp(0i32, out_height as i32) as u32;

    let lut_left = tile_xs.0.clamp(0, n_tiles_wh.0 as i32 - 1) as usize;
    let lut_right = tile_xs.1.clamp(0, n_tiles_wh.0 as i32 - 1) as usize;
    let lut_top = tile_ys.0.clamp(0, n_tiles_wh.1 as i32 - 1) as usize;
    let lut_bottom = tile_ys.1.clamp(0, n_tiles_wh.1 as i32 - 1) as usize;

    let hist_00 = &luts[lut_left + n_tiles_wh.0 * lut_top];
    let hist_10 = &luts[lut_right + n_tiles_wh.0 * lut_top];
    let hist_01 = &luts[lut_left + n_tiles_wh.0 * lut_bottom];
    let hist_11 = &luts[lut_right + n_tiles_wh.0 * lut_bottom];
    let scale = U_MAX as f32 / T_MAX as f32;

    for (xi, x) in (x_start..x_end).enumerate() {
        for (yi, y) in (y_start..y_end).enumerate() {
            let xw = xi as f32 / tile_width as f32;
            let yw = yi as f32 / tile_height as f32;
            let w_00 = (1.0 - xw) * (1.0 - yw);
            let w_10 = xw * (1.0 - yw);
            let w_01 = (1.0 - xw) * yw;
            let w_11 = xw * yw;

            let p: usize = input.get_pixel(x, y).0[0].to_usize().unwrap_or(0);

            let q = (scale
                * (hist_00[p] as f32 * w_00
                    + hist_01[p] as f32 * w_01
                    + hist_10[p] as f32 * w_10
                    + hist_11[p] as f32 * w_11))
                .clamp(0.0, U::max_value().to_f32().unwrap_or(0.0));
            let q: U = U::from_f32(q).unwrap_or(U::zero());

            debug_assert!((w_00 + w_10 + w_01 + w_11 - 1.0).abs() < 0.0001);
            dst.put_pixel(x, y, Luma([q]));
        }
    }
}

pub fn clahe_generic<T, U, const T_MAX: usize, const U_MAX: usize>(
    tiles_x: usize,
    tiles_y: usize,
    clip_limit: f32,
    input: &ImageBuffer<Luma<T>, Vec<T>>,
) -> ImageBuffer<Luma<U>, Vec<U>>
where
    T: image::Primitive,
    U: image::Primitive
        + num_traits::cast::ToPrimitive
        + num_traits::cast::FromPrimitive
        + std::fmt::Display,
{
    assert!(tiles_x > 0, "tiles_x must be > 0");
    assert!(tiles_y > 0, "tiles_y must be > 0");
    let mut dst = ImageBuffer::<Luma<U>, Vec<U>>::new(input.width(), input.height());
    let mut _store = None;

    let (tile_size_wh, src_for_lut) = if input.width().is_multiple_of(tiles_x as u32)
        && input.height().is_multiple_of(tiles_y as u32)
    {
        (
            (
                input.width() as usize / tiles_x,
                input.height() as usize / tiles_y,
            ),
            input,
        )
    } else {
        let tile_width = (input.width() as usize).div_ceil(tiles_x);
        let tile_height = (input.height() as usize).div_ceil(tiles_y);
        let new_width = tile_width * tiles_x;
        let new_height = tile_height * tiles_y;
        let max_x = input.width() as i32 - 1;
        let max_y = input.height() as i32 - 1;
        let img = ImageBuffer::from_fn(new_width as u32, new_height as u32, |x, y| {
            // mirror boundary
            // max_x - abs(0 - max_x) => 0
            // max_x - abs(width - 1 - max_x) => width - 1
            // max_x - abs(width - max_x) => width - 2
            // max_x - abs(width + 1 - max_x) => width - 3

            let src_x = (max_x - (x as i32 - max_x).abs()) as u32;
            let src_y = (max_y - (y as i32 - max_y).abs()) as u32;
            *input.get_pixel(src_x, src_y)
        });

        _store = Some(img);
        ((tile_width, tile_height), _store.as_ref().unwrap())
    };

    let tile_size_total = tile_size_wh.0 * tile_size_wh.1;
    let lut_scale = (T_MAX as f32 - 1.0) / tile_size_total as f32;

    let clip_limit = if clip_limit > 0.0 {
        (clip_limit * tile_size_total as f32 / T_MAX as f32).max(1.0) as i32
    } else {
        0
    };

    // TODO is there a parallel for solution in rust?
    let mut luts: Vec<[u32; T_MAX]> = vec![[0; T_MAX]; tiles_x * tiles_y];
    for tile_x in 0..tiles_x {
        for tile_y in 0..tiles_y {
            calc_lut_body::<T, T_MAX>(
                &mut luts[tile_y * tiles_x + tile_x],
                src_for_lut,
                tile_size_wh,
                clip_limit,
                lut_scale,
                tile_x,
                tile_y,
            );
        }
    }

    // Produce pairs of (None, 0), (0, 1) ... (n-1, None)
    // in both x and y, each interpolate will take a mixture of the two
    // or in the case of boundaries of just its one
    for tile_x in 0..=tiles_x {
        for tile_y in 0..=tiles_y {
            interpolate::<T, U, T_MAX, U_MAX>(
                &mut dst,
                src_for_lut,
                &luts,
                tile_size_wh,
                (tiles_x, tiles_y),
                (tile_x as i32 - 1, tile_x as i32),
                (tile_y as i32 - 1, tile_y as i32),
            );
        }
    }

    dst
}

pub fn clahe_u8_to_u8(
    tiles_x: usize,
    tiles_y: usize,
    clip_limit: f32,
    input: &GrayImage,
) -> GrayImage {
    clahe_generic::<u8, u8, 256, 256>(tiles_x, tiles_y, clip_limit, input)
}

pub fn clahe_u16_to_u8(
    tiles_x: usize,
    tiles_y: usize,
    clip_limit: f32,
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
) -> GrayImage {
    clahe_generic::<u16, u8, 65536, 256>(tiles_x, tiles_y, clip_limit, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_image_produces_uniform_output() {
        let img = ImageBuffer::from_pixel(64, 64, Luma([128u8]));
        let out = clahe_u8_to_u8(8, 8, 2.0, &img);
        let first = out.pixels().next().unwrap().0[0];
        for p in out.pixels() {
            assert_eq!(p.0[0], first, "all pixels should be the same value");
        }
    }

    #[test]
    fn output_dimensions_match_input() {
        let img = ImageBuffer::from_fn(100, 80, |x, _y| Luma([(x % 256) as u8]));
        let out = clahe_u8_to_u8(8, 8, 2.0, &img);
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 80);
    }

    #[test]
    fn non_divisible_dimensions() {
        let img = ImageBuffer::from_fn(97, 53, |x, y| Luma([((x + y) % 256) as u8]));
        let out = clahe_u8_to_u8(8, 8, 2.0, &img);
        assert_eq!(out.width(), 97);
        assert_eq!(out.height(), 53);
    }

    #[test]
    fn single_tile() {
        let img = ImageBuffer::from_fn(64, 64, |x, _y| Luma([(x * 4) as u8]));
        let out = clahe_u8_to_u8(1, 1, 2.0, &img);
        assert_eq!(out.width(), 64);
        assert_eq!(out.height(), 64);
    }

    #[test]
    fn u16_to_u8_produces_valid_output() {
        let img: ImageBuffer<Luma<u16>, Vec<u16>> =
            ImageBuffer::from_fn(128, 128, |x, y| Luma([((x * y) % 65536) as u16]));
        let out = clahe_u16_to_u8(4, 4, 3.0, &img);
        assert_eq!(out.width(), 128);
        assert_eq!(out.height(), 128);
        let max_val = out.pixels().map(|p| p.0[0]).max().unwrap();
        assert!(max_val > 0, "output should not be all zeros");
    }

    #[test]
    fn zero_clip_limit_no_clipping() {
        let img = ImageBuffer::from_fn(64, 64, |x, y| Luma([((x + y) % 256) as u8]));
        let out = clahe_u8_to_u8(8, 8, 0.0, &img);
        assert_eq!(out.width(), 64);
        assert_eq!(out.height(), 64);
    }

    #[test]
    fn u16_to_u8_output_dimensions() {
        let img: ImageBuffer<Luma<u16>, Vec<u16>> =
            ImageBuffer::from_fn(64, 64, |x, _y| Luma([(x * 256) as u16]));
        let out = clahe_u16_to_u8(4, 4, 2.0, &img);
        assert_eq!(out.width(), 64);
        assert_eq!(out.height(), 64);
    }

    #[test]
    fn gradient_uses_full_range() {
        let img = ImageBuffer::from_fn(128, 128, |x, _y| Luma([((x * 2) % 256) as u8]));
        let out = clahe_u8_to_u8(4, 4, 40.0, &img);
        let out_min = out.pixels().map(|p| p.0[0]).min().unwrap();
        let out_max = out.pixels().map(|p| p.0[0]).max().unwrap();
        assert!(out_max > 200, "expected high max, got {}", out_max);
        assert!(out_min < 55, "expected low min, got {}", out_min);
    }
}
