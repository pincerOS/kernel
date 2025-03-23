use gfx::format::pcf::LoadedPCF;
use gfx::{color, format};

use crate::__include_bytes_align;
use crate::device::MAILBOX;

use super::mailbox::Surface;

#[path = "../../../console/src/grid.rs"]
mod grid;
#[path = "../../../console/src/vt100.rs"]
mod vt100;

pub struct Console {
    surface: Surface,
    grid: grid::CharGrid,
    emulator: vt100::EmulatorState,
    font: LoadedPCF<'static>,
}

pub fn init() -> Console {
    let font_data = __include_bytes_align!(u32, "../../../console/ctrld-fixed-10r.pcf");
    let font = format::pcf::load_pcf(font_data).unwrap();

    let surface = unsafe { MAILBOX.get().lock().map_framebuffer_kernel(640, 480) };
    let (width, height) = surface.dimensions();

    // let char_dims = font.dimensions();
    let scale = 1;
    let hpad = 2;
    let vpad = 0;

    let fill_color = grid::Colors {
        fg: color::rgba(255, 255, 255, 255),
        bg: color::rgba(0, 0, 0, 0),
    };
    let grid = grid::CharGrid::new(
        (width, height),
        font.dimensions(),
        scale,
        hpad,
        vpad,
        fill_color,
    );
    let emulator = vt100::EmulatorState::new(grid.rows, grid.cols);

    Console {
        surface,
        grid,
        emulator,
        font,
    }
}

impl Console {
    pub fn input(&mut self, data: &[u8]) {
        self.emulator.input(data);
    }
    pub fn render(&mut self) {
        let row_stride = self.surface.stride();
        self.emulator
            .update(self.grid.region(0, 0, self.grid.rows, self.grid.cols));

        let data = self.surface.buffer();
        data.fill(color::rgba(0, 0, 0, 255));
        render_grid(
            &self.grid,
            self.grid.char_dims,
            self.grid.scale,
            self.grid.vpad,
            self.grid.hpad,
            data,
            row_stride,
            &self.font,
        );
        self.surface.present();
    }
}

fn render_grid(
    grid: &grid::CharGrid,
    char_dims: (usize, usize),
    scale: usize,
    vpad: usize,
    hpad: usize,
    data: &mut [u32],
    row_stride: usize,
    font: &format::pcf::LoadedPCF<'_>,
) {
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            let ch = grid.chars[r * grid.cols + c];
            let colors = grid.colors[r * grid.cols + c];

            let y = char_dims.0 * scale * r + vpad;
            let x = char_dims.1 * scale * c + hpad;

            if (colors.bg & 0xFF000000) != 0 {
                for pr in 0..char_dims.0 * scale {
                    for pc in 0..char_dims.1 * scale {
                        let pixel = &mut data[(y + pr) * row_stride + x + pc];
                        *pixel = gfx::color::blend(colors.bg, *pixel);
                    }
                }
            }

            if ch != ' ' && (colors.fg & 0xFF000000) != 0 {
                font.draw_char(ch, data, y * row_stride + x, row_stride, scale, colors.fg);
            }
        }
    }
}
