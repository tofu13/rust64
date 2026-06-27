// bitmap font used in debugger window
use utils;

pub struct SysFont {
    data: Vec<u8>
}


impl SysFont {
    pub fn new() -> SysFont {
        let mut font = SysFont {
            data: Vec::<u8>::new(),
        };

        // lazily skipping the BMP header to actual data
        let bmp_data = utils::open_file("res/font.bmp", 54);

        let mut j: i32 = 256*63*3;
        let mut i;

        while j >= 0 {
            i = 0;
            while i < 256 * 3 {
                let color = if bmp_data[i + j as usize] != 0 { 1 } else { 0 };
                font.data.push(color);
                i+= 3;
            }
            
            j -= 256 * 3;
        }
        
        font
    }

    pub fn draw_text_rgb(&self, window_buffer: &mut Vec<u32>, window_w: usize, x: usize, y: usize, text: &str, color: u32) {
        let chars: Vec<char> = text.chars().collect();
        for i in 0..text.len() {
            self.draw_char_rgb(window_buffer, window_w, x*8 + 8*i as usize, y*8 as usize, utils::ascii_to_petscii(chars[i]), color);
        }
    }    
    
    pub fn draw_text(&self, window_buffer: &mut Vec<u32>, window_w: usize, x: usize, y: usize, text: &str, c64_color: u8) {
        let chars: Vec<char> = text.chars().collect();
        for i in 0..text.len() {
            self.draw_char(window_buffer, window_w, x*8 + 8*i as usize, y*8 as usize, utils::ascii_to_petscii(chars[i]), c64_color);
        }
    }
    
    pub fn draw_char(&self, window_buffer: &mut Vec<u32>, window_w: usize, x: usize, y: usize, charcode: u8, c64_color: u8) {
        self.draw_char_rgb(window_buffer, window_w, x, y, charcode, utils::fetch_c64_color_rgba(c64_color));
    }

    pub fn draw_char_rgb(&self, window_buffer: &mut Vec<u32>, window_w: usize, x: usize, y: usize, charcode: u8, color: u32) {
        let char_w: i32 = 8;
        let char_h: i32 = 8;
        let data_x = char_w * (charcode % 32) as i32;
        let data_y = char_h * (charcode / 32) as i32;
        let data_w = data_x + char_w;
        let data_h = data_y + char_h;

        let mut k = 0;
        let mut l = 0;
        for i in data_y..data_h {
            for j in data_x..data_w {
                window_buffer[x + l + window_w * (y + k)] = self.data[j as usize + (i * 256) as usize] as u32 * color;
                l += 1;
            }
            l = 0;
            k += 1;
        }
    }
}
