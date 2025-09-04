fn main() {
    let (in_file, out_file) = if std::env::args().count() == 3 {
        (
            std::env::args().nth(1).unwrap(),
            std::env::args().nth(2).unwrap(),
        )
    } else {
        panic!(
            "Usage: {} <input> <output>",
            std::env::args().nth(0).unwrap()
        );
    };

    let im = image::open(&in_file).unwrap();
    let im = im.into_luma16();
    let output = clahe::clahe_u16_to_u8(8, 8, 0.0, &im).unwrap();

    output
        .save_with_format(&out_file, image::ImageFormat::Png)
        .unwrap();
}
