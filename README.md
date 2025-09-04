# clahe-rs

Reimplementation of opencv's CLAHE in pure rust. This includes implementing u16
-> u8 conversion.

## 8bit

![Before Processing](honeycomb.png)

![After Processing](honeycomp_out.png)

## 16bit

![Before Processing](high_depth.png)

![After Processing](high_depth_out.png)

![Before Processing](fractal.png)

![After Processing](fractal_out.png)

# Future work

- Rayon threading
