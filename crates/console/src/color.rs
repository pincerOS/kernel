pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    u32::from_be_bytes([a, r, g, b])
}

pub const fn de_rgba(color: u32) -> (u8, u8, u8, u8) {
    let [a, r, g, b] = color.to_be_bytes();
    (r, g, b, a)
}

pub fn blend(one: u32, two: u32) -> u32 {
    let [a, r, g, b] = one.to_be_bytes().map(u32::from);
    if a == 255 {
        return one;
    }
    let [_bg_a, bg_r, bg_g, bg_b] = two.to_be_bytes().map(u32::from);
    // TODO: blend in linear space?
    let (r, g, b) = (
        (r * a + bg_r * (255 - a)) / 255,
        (g * a + bg_g * (255 - a)) / 255,
        (b * a + bg_b * (255 - a)) / 255,
    );
    u32::from_be_bytes([a, r, g, b].map(|b| b as u8))
}
