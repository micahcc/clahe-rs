use std::path::Path;

fn main() {
    let file = if std::env::args().count() == 2 {
        std::env::args().nth(1).unwrap()
    } else {
        panic!("Please enter an image filename")
    };

    let im = image::open(&Path::new(&file)).unwrap();
    let output = clahe_rs::clahe_u8_to_u8(8, 8, 2.0, im.as_luma8().unwrap()).unwrap();

    output
        .save_with_format(&Path::new("output.png"), image::ImageFormat::Png)
        .unwrap();
}
