use super::cell::CellData;
use rayon::prelude::*;

const PARALLEL_CELL_THRESHOLD: usize = 50_000;

pub struct FrameProcessor {
    pub width: usize,
    pub height: usize,
}

impl FrameProcessor {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    #[cfg(test)]
    pub fn process_frame(&self, pixel_data: &[u8]) -> Vec<CellData> {
        let mut cells = vec![CellData::default(); self.width * (self.height / 2)];
        self.process_frame_into(pixel_data, &mut cells);
        cells
    }

    pub fn process_frame_into(&self, pixel_data: &[u8], cells: &mut [CellData]) {
        let w = self.width;
        let h = self.height;
        let term_height = h / 2;
        let cell_count = w * term_height;

        if cells.len() != cell_count {
            return;
        }

        if cell_count < PARALLEL_CELL_THRESHOLD {
            for (idx, cell) in cells.iter_mut().enumerate() {
                write_cell(pixel_data, w, idx, cell);
            }
            return;
        }

        let chunk_size = 2000;

        cells
            .par_chunks_mut(chunk_size)
            .enumerate()
            .for_each(|(chunk_idx, chunk)| {
                let start_idx = chunk_idx * chunk_size;

                for (i, cell) in chunk.iter_mut().enumerate() {
                    let idx = start_idx + i;
                    write_cell(pixel_data, w, idx, cell);
                }
            });
    }
}

fn write_cell(pixel_data: &[u8], width: usize, idx: usize, cell: &mut CellData) {
    let cx = idx % width;
    let cy = idx / width;

    let py_top = cy * 2;
    let py_bottom = cy * 2 + 1;

    let (tr, tg, tb) = get_pixel(pixel_data, width, cx, py_top);
    let (br, bg, bb) = get_pixel(pixel_data, width, cx, py_bottom);

    *cell = CellData {
        char: '▀',
        fg: (tr, tg, tb),
        bg: (br, bg, bb),
    };
}

fn get_pixel(pixel_data: &[u8], width: usize, x: usize, y: usize) -> (u8, u8, u8) {
    let offset = (y * width + x) * 3;
    if offset + 2 < pixel_data.len() {
        (
            pixel_data[offset],
            pixel_data[offset + 1],
            pixel_data[offset + 2],
        )
    } else {
        (0, 0, 0)
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
