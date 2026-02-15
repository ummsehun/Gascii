use super::cell::CellData;
use rayon::prelude::*;

pub struct FrameProcessor {
    pub width: usize,
    pub height: usize,
}

impl FrameProcessor {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub fn process_frame(&self, pixel_data: &[u8]) -> Vec<CellData> {
        let mut cells = vec![CellData::default(); self.width * (self.height / 2)];
        self.process_frame_into(pixel_data, &mut cells);
        cells
    }

    pub fn process_frame_into(&self, pixel_data: &[u8], cells: &mut [CellData]) {
        let w = self.width;
        let h = self.height;
        let term_height = h / 2;

        if cells.len() != w * term_height {
            return;
        }

        let row_stride = w * 3;
        let required_bytes = h * row_stride;
        if pixel_data.len() < required_bytes {
            return;
        }

        // Small frames are memory-bound; single-threaded row walk is often faster.
        if term_height < 48 {
            for (cy, row_cells) in cells.chunks_mut(w).enumerate() {
                process_row(row_cells, pixel_data, cy, row_stride);
            }
            return;
        }

        cells
            .par_chunks_mut(w)
            .enumerate()
            .for_each(|(cy, row_cells)| process_row(row_cells, pixel_data, cy, row_stride));
    }
}

#[inline(always)]
fn process_row(row_cells: &mut [CellData], pixel_data: &[u8], cy: usize, row_stride: usize) {
    let top_row_offset = cy * 2 * row_stride;
    let bottom_row_offset = top_row_offset + row_stride;

    for (cx, cell) in row_cells.iter_mut().enumerate() {
        let top = top_row_offset + cx * 3;
        let bottom = bottom_row_offset + cx * 3;

        *cell = CellData {
            char: '▀',
            fg: (pixel_data[top], pixel_data[top + 1], pixel_data[top + 2]),
            bg: (
                pixel_data[bottom],
                pixel_data[bottom + 1],
                pixel_data[bottom + 2],
            ),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_frame_half_block() {
        let proc = FrameProcessor::new(2, 4);
        let mut frame = vec![0u8; 2 * 4 * 3];
        // (0,0) red
        frame[0] = 255;
        frame[1] = 0;
        frame[2] = 0;
        // (1,0) red
        frame[3] = 255;
        frame[4] = 0;
        frame[5] = 0;
        // (0,1) green
        frame[6] = 0;
        frame[7] = 255;
        frame[8] = 0;
        // (1,1) green
        frame[9] = 0;
        frame[10] = 255;
        frame[11] = 0;
        // (0,2) blue
        frame[12] = 0;
        frame[13] = 0;
        frame[14] = 255;
        // (1,2) blue
        frame[15] = 0;
        frame[16] = 0;
        frame[17] = 255;
        // (1,3) yellow
        frame[18] = 255;
        frame[19] = 255;
        frame[20] = 0;
        // (1,3) yellow
        frame[21] = 255;
        frame[22] = 255;
        frame[23] = 0;

        let cells = proc.process_frame(&frame);
        assert_eq!(cells.len(), 2 * 2);

        // Check if colors are mapped correctly
        assert_eq!(cells[0].fg, (255, 0, 0));
        assert_eq!(cells[0].bg, (0, 255, 0));
    }
}
