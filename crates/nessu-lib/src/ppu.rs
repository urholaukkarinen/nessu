use std::mem::transmute;

use crate::bitwise::HasBits;
use crate::cartridge::Cartridge;
use crate::mapper::Mirroring;

const DISPLAY_BYTES: usize = 245760;
pub const DEFAULT_PALETTE: &[(u8, u8, u8); 64] =
    unsafe { transmute(include_bytes!("../../../default.pal") as &[u8; 192]) };

const NAMETABLE_X_BITS: u16 = 0b000_0100_0000_0000;
const NAMETABLE_Y_BITS: u16 = 0b000_1000_0000_0000;
const NAMETABLE_BITS: u16 = NAMETABLE_X_BITS | NAMETABLE_Y_BITS;
const TILE_X_BITS: u16 = 0b000_0000_0001_1111;
const TILE_Y_BITS: u16 = 0b000_0011_1110_0000;
const PIXEL_Y_BITS: u16 = 0b111_0000_0000_0000;
const VBL_PPU_CYCLE: u128 = 82182;

#[derive(Copy, Clone)]
pub struct Sprite {
    idx: u8,
    active: bool,
    x: u8,
    y: u8,
    tile_idx: u8,
    attrs: u8,
    tile_lo: u8,
    tile_hi: u8,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            idx: 0xFF,
            active: false,
            x: 0xFF,
            y: 0xFF,
            tile_idx: 0,
            attrs: 0,
            tile_lo: 0,
            tile_hi: 0,
        }
    }
}

pub struct Ppu {
    cart: *mut Cartridge,

    vram: Vec<u8>,

    pub ppu_ctrl: u8,
    pub ppu_mask: u8,
    pub ppu_status: u8,
    pub ppu_addr: u16,

    pub vram_addr: u16,

    pub ppu_data_buf: u8,

    pub oam_addr: u8,
    pub primary_oam: [u8; 256],
    pub secondary_oam: [Sprite; 8],
    pub active_sprites: [Sprite; 8],

    pub pixel_x: u8,

    w_toggle: bool,

    vbl_cycle_counter: u128,

    nmi_triggered: bool,
    suppress_next_nmi: bool,

    shift_bg_tile_lo: u16,
    shift_bg_tile_hi: u16,
    shift_bg_attr_lo: u16,
    shift_bg_attr_hi: u16,

    next_nt_tile: u8,
    next_attr_tile: u8,
    next_bg_tile_lo: u8,
    next_bg_tile_hi: u8,

    sprite_evaluation_idx: usize,
    found_sprites: usize,

    cycle: u16,
    scanline: u16,

    odd_frame: bool,

    pub display: Vec<u8>,

    pub open_bus: u8,
    pub open_bus_decay_timer: u32,

    pub a12_timer: u8,

    pub sprite_rendering_enabled_by_user: bool,
    pub bg_rendering_enabled_by_user: bool,
}

impl Ppu {
    pub fn new(cart: *mut Cartridge) -> Self {
        Self {
            cart,

            vram: vec![0; 0x4000],

            ppu_ctrl: 0,
            ppu_mask: 0,
            ppu_status: 0,
            ppu_addr: 0,
            vram_addr: 0,
            ppu_data_buf: 0,
            oam_addr: 0,
            primary_oam: [0; 256],
            secondary_oam: [Default::default(); 8],
            active_sprites: [Default::default(); 8],
            pixel_x: 0,
            w_toggle: false,
            vbl_cycle_counter: 0,
            nmi_triggered: false,
            suppress_next_nmi: false,
            shift_bg_tile_lo: 0,
            shift_bg_tile_hi: 0,
            shift_bg_attr_lo: 0,
            shift_bg_attr_hi: 0,
            next_nt_tile: 0,
            next_attr_tile: 0,
            next_bg_tile_lo: 0,
            next_bg_tile_hi: 0,
            sprite_evaluation_idx: 0,
            found_sprites: 0,
            cycle: 0,
            odd_frame: false,
            scanline: 0,
            display: vec![0; DISPLAY_BYTES],
            open_bus: 0,
            open_bus_decay_timer: 0,
            a12_timer: 0,

            sprite_rendering_enabled_by_user: true,
            bg_rendering_enabled_by_user: true,
        }
    }

    pub fn reset(&mut self, cart: *mut Cartridge) {
        *self = Ppu {
            oam_addr: self.oam_addr,
            ppu_addr: self.ppu_addr,
            ppu_status: self.ppu_status & 0x80,
            open_bus: self.open_bus,
            open_bus_decay_timer: self.open_bus_decay_timer,
            ..Ppu::new(cart)
        }
    }

    fn cart_mut(&mut self) -> &mut Cartridge {
        unsafe { &mut *self.cart }
    }

    pub fn current_cycle(&self) -> u16 {
        self.cycle
    }

    pub fn current_scanline(&self) -> u16 {
        self.scanline
    }

    #[inline]
    pub fn frame_completed(&self) -> bool {
        self.scanline == 0 && self.cycle == 0
    }

    pub fn background_pattern_table_address(&self) -> u16 {
        match (self.ppu_ctrl >> 4) & 1 {
            0 => 0x0000,
            1 => 0x1000,
            _ => unreachable!(),
        }
    }

    fn sprite_pattern_table_address(&self) -> u16 {
        match (self.ppu_ctrl >> 3) & 1 {
            0 => 0x0000,
            1 => 0x1000,
            _ => unreachable!(),
        }
    }

    fn use_large_sprites(&self) -> bool {
        self.ppu_ctrl.has_bits(0b10_0000)
    }

    pub fn read_ppu_status(&mut self, read_only: bool) -> u8 {
        let mut status = self.ppu_status;

        if !read_only {
            self.w_toggle = false;
            self.ppu_status &= 0x7F;

            if self.vbl_cycle_counter == VBL_PPU_CYCLE - 1 {
                status &= 0x7F;
                // suppress next nmi
                self.suppress_next_nmi = true;
            } else if self.vbl_cycle_counter == VBL_PPU_CYCLE
                || self.vbl_cycle_counter == VBL_PPU_CYCLE + 1
            {
                // suppress current nmi
                self.nmi_triggered = false;
            }
        }

        (status & 0xE0) | (self.open_bus & 0x1F)
    }

    pub fn write_ppu_ctrl(&mut self, val: u8) {
        let nmi_enable_toggled_on = val.has_bits(0x80) && !self.ppu_ctrl.has_bits(0x80);

        self.ppu_ctrl = val;

        // Write nametable selection to PPUADDR
        self.ppu_addr = (self.ppu_addr & 0b1111_0011_1111_1111) | (((val & 0b11) as u16) << 10);

        if nmi_enable_toggled_on && self.ppu_status.has_bits(0x80) {
            self.nmi_triggered = true;
        }

        if !self.ppu_ctrl.has_bits(0x80)
            && (VBL_PPU_CYCLE - 1..=VBL_PPU_CYCLE + 1).contains(&self.vbl_cycle_counter)
        {
            // NMI should not occur if disabled too close to VBL start
            self.nmi_triggered = false;
        }

        self.write_open_bus(val, true);
    }

    pub fn write_ppu_mask(&mut self, val: u8) {
        self.ppu_mask = val;
        self.write_open_bus(val, true);
    }

    pub fn write_ppu_scroll(&mut self, val: u8) {
        if !self.w_toggle {
            self.pixel_x = val & 0b111;
            self.ppu_addr = (self.ppu_addr & 0b111_1111_1110_0000) | ((val as u16) >> 3);
        } else {
            self.ppu_addr = (((val & 0b111) as u16) << 12)
                | (((val & 0b1111_1000) as u16) << 2)
                | (self.ppu_addr & 0b000_1100_0001_1111);
        }
        self.w_toggle = !self.w_toggle;

        self.write_open_bus(val, true);
    }

    pub fn write_ppu_addr(&mut self, val: u8) {
        if !self.w_toggle {
            self.ppu_addr = (self.ppu_addr & 0x00FF) | (((val & 0b0011_1111) as u16) << 8);
        } else {
            self.ppu_addr = (self.ppu_addr & 0xFF00) | (val as u16);
            self.update_vram_addr(self.ppu_addr);
        }
        self.w_toggle = !self.w_toggle;
        self.write_open_bus(val, true);
    }

    pub fn nmi_triggered(&mut self) -> bool {
        let val = self.nmi_triggered;
        self.nmi_triggered = false;
        val
    }

    pub fn increment_vram_addr(&mut self) {
        self.update_vram_addr(
            self.vram_addr
                + if self.ppu_ctrl.has_bits(0b0100) {
                    32
                } else {
                    1
                },
        );
    }

    fn update_vram_addr(&mut self, new_addr: u16) {
        if self.a12_timer >= 8 && new_addr.has_bits(0x1000) {
            self.cart_mut().clock_irq();
        }

        if !self.vram_addr.has_bits(0x1000) {
            self.a12_timer = self.a12_timer.saturating_add(1);
        } else {
            self.a12_timer = 0;
        }

        self.vram_addr = new_addr;
    }

    #[inline]
    fn background_rendering_enabled(&self) -> bool {
        self.ppu_mask.has_bits(0b0_1000)
    }

    #[inline]
    fn sprite_rendering_enabled(&self) -> bool {
        self.ppu_mask.has_bits(0b1_0000)
    }

    #[inline]
    fn rendering_enabled(&self) -> bool {
        self.background_rendering_enabled() || self.sprite_rendering_enabled()
    }

    pub fn write_oam_data(&mut self, val: u8) {
        // TODO: Writes during rendering do not modify values in OAM, but do perform
        // a glitchy increment of OAMADDR, bumping only the high 6 bits
        // https://www.nesdev.org/wiki/PPU_registers#OAMDATA

        self.primary_oam[self.oam_addr as usize] = val;
        self.oam_addr = self.oam_addr.wrapping_add(1);
        self.write_open_bus(val, true);
    }

    pub fn write_oam_addr(&mut self, val: u8) {
        self.oam_addr = val;
        self.write_open_bus(val, true);
    }

    pub fn write_open_bus(&mut self, val: u8, refresh: bool) {
        if refresh && val != 0 {
            self.open_bus_decay_timer = 5360520;
        }

        self.open_bus = val;
    }

    fn secondary_oam_clear(&mut self) {
        self.secondary_oam = Default::default();
    }

    pub fn clock(&mut self) {
        self.update_open_bus();

        if self.cycle == 0 && self.scanline == 0 {
            self.vbl_cycle_counter = 0;
        } else {
            self.vbl_cycle_counter += 1;
        }

        if self.cycle == 0
            && self.scanline == 0
            && self.odd_frame
            && self.background_rendering_enabled()
        {
            self.cycle += 1;
            self.vbl_cycle_counter += 1;
        }

        // Visible scanlines only
        if let 0..=239 = self.scanline {
            if let 1..=64 = self.cycle {
                self.secondary_oam_clear();
                self.clear_sprite_overflow();
            }

            if let 65..=256 = self.cycle {
                self.sprite_evaluation();
            }
        }

        // Visible and pre-render scanlines
        if let 0..=239 | 261 = self.scanline {
            if self.cycle == 257 {
                // Garbage nt byte
                self.load_nametable_byte();
            }

            if self.cycle == 259 {
                // Garbage at byte
                self.load_attribute_table_byte();
            }

            if let 1..=256 | 321..=337 = self.cycle {
                if self.cycle > 1 {
                    self.advance_bg_shifters();
                }

                match self.cycle % 8 {
                    1 => {
                        if self.cycle > 1 {
                            self.load_bg_shift_registers();
                        }
                        self.load_nametable_byte();
                    }
                    3 => {
                        self.load_attribute_table_byte();
                    }
                    5 => {
                        self.load_low_bg_tile_byte();
                    }
                    6 => {
                        self.load_high_bg_tile_byte();
                    }
                    0 => {
                        self.increment_scroll_x();
                    }
                    _ => {}
                }
            }

            // Visible cycles
            if let (1..=256, 0..=239) = (self.cycle, self.scanline) {
                self.draw_pixel();
            }

            // Increment Y at the end of a scanline
            if self.cycle == 256 {
                self.increment_scroll_y();
            }

            if self.cycle == 257 {
                self.reload_horizontal_scroll_bits();
                self.active_sprites = self.secondary_oam;
            }

            if self.cycle == 337 || self.cycle == 339 {
                // Garbage nt byte
                self.load_nametable_byte();
            }

            if let 261..=320 = self.cycle {
                match self.cycle & 7 {
                    1 => {
                        // Garbage nt byte
                        self.load_nametable_byte();
                    }
                    3 => {
                        // Garbage at byte
                        self.load_attribute_table_byte();
                    }
                    5 => {
                        self.load_low_sprite_tile_byte();
                    }
                    6 => {
                        self.load_high_sprite_tile_byte();
                    }
                    _ => {}
                }
            }

            if let 256..=320 = self.cycle {
                self.oam_addr = 0;
            }
        }

        // V-Blank
        if self.scanline == 241 && self.cycle == 1 {
            self.set_vblank_status();
        }

        if self.scanline == 261 {
            match self.cycle {
                1 => {
                    self.clear_vblank_status();
                    self.clear_sprite_zero_hit();
                }
                280..=304 => {
                    self.reload_vertical_scroll_bits();
                }
                _ => {}
            }
        }

        self.cycle += 1;

        if self.cycle >= 341 {
            self.sprite_evaluation_idx = 0;
            self.found_sprites = 0;
            self.cycle = 0;
            self.scanline += 1;

            if self.scanline >= 262 {
                self.scanline = 0;
                self.odd_frame = !self.odd_frame;
            }
        }
    }

    fn update_open_bus(&mut self) {
        if self.open_bus_decay_timer == 0 {
            self.open_bus = 0;
        } else {
            self.open_bus_decay_timer -= 1;
        }
    }

    fn sprite_evaluation(&mut self) {
        if self.sprite_evaluation_idx >= 64 {
            // TODO? If n has overflowed back to zero (all 64 sprites evaluated)
            // Attempt (and fail) to copy OAM[n][0] into the next free slot in secondary OAM
            // and increment n (repeat until HBLANK is reached)
            return;
        }

        let primary_oam_idx = (self.sprite_evaluation_idx & 0x3F) << 2;
        let sprite_y = self.primary_oam[primary_oam_idx].saturating_add(1);

        let next_y = self.scanline + 1;
        let sprite_height = if self.use_large_sprites() { 16 } else { 8 };

        let in_range = next_y > 0
            && next_y < 0xF0
            && next_y >= sprite_y as u16
            && next_y < sprite_y as u16 + sprite_height;

        if self.found_sprites < 8 {
            let sprite = &mut self.secondary_oam[self.found_sprites];
            sprite.y = sprite_y;
            sprite.idx = self.sprite_evaluation_idx as u8;

            if in_range {
                sprite.active = true;
                sprite.x = self.primary_oam[primary_oam_idx + 3];
                sprite.attrs = self.primary_oam[primary_oam_idx + 2];
                sprite.tile_idx = self.primary_oam[primary_oam_idx + 1];

                self.found_sprites += 1;
            }
        } else if in_range {
            self.set_sprite_overflow();
        }

        self.sprite_evaluation_idx += 1;
    }

    pub fn read_ppu_data(&mut self, read_only: bool) -> u8 {
        let prev_val = self.ppu_data_buf;
        let curr_val = self.read_mem_u8(self.vram_addr);

        let is_palette = self.vram_addr >= 0x3F00;

        if !read_only {
            // Reading palette data from $3F00-$3FFF works differently.
            // The palette data is placed immediately on the data bus, and hence no
            // priming read is required. Reading the palettes still updates the internal
            // buffer though, but the data placed in it is the mirrored nametable data
            // that would appear "underneath" the palette.
            if is_palette {
                self.ppu_data_buf = self.read_mem_u8(self.vram_addr & 0x2FFF);
            } else {
                self.ppu_data_buf = curr_val;
            }
            self.increment_vram_addr();
        }

        // When reading while the VRAM address is in the range 0-$3EFF, the read will
        // return the contents of an internal read buffer.
        let val = if is_palette {
            (curr_val & 0x3F) | (self.open_bus & 0xC0)
        } else {
            prev_val
        };

        self.write_open_bus(val, false);
        val
    }

    pub fn write_vram(&mut self, val: u8) {
        self.write_mem_u8(self.vram_addr, val);
        self.increment_vram_addr();
        self.write_open_bus(val, true);
    }

    pub fn read_mem_u8(&mut self, addr: u16) -> u8 {
        let addr = self.effective_addr(addr) as usize;

        self.cart_mut()
            .ppu_read_u8(addr)
            .unwrap_or_else(|| self.vram[addr as usize])
    }

    pub fn write_mem_u8(&mut self, addr: u16, val: u8) {
        let addr = self.effective_addr(addr) as usize;

        if !self.cart_mut().ppu_write_u8(addr, val) {
            self.vram[addr as usize] = val;
        }
    }

    fn effective_addr(&mut self, addr: u16) -> u16 {
        let mirroring = self.cart_mut().mirroring();

        let addr = addr & 0x3FFF;
        match addr {
            0x2000..=0x3EFF => {
                0x2000
                    | match mirroring {
                        Mirroring::OneScreenLowerBank => addr & 0x03FF,
                        Mirroring::OneScreenUpperBank => addr & 0x03FF | 0x0400,
                        Mirroring::Horizontal => {
                            if addr & 0xFFF < 0x800 {
                                addr & 0x03FF
                            } else {
                                addr & 0x03FF | 0x0400
                            }
                        }
                        Mirroring::Vertical => addr & 0x07FF,
                    }
            }

            0x3F10 | 0x3F14 | 0x3F18 | 0x3F1C | 0x3F30 | 0x3F34 | 0x3F38 | 0x3F4C => addr & 0x3F0F,
            0x3F20..=0x3FFF => addr & 0x3F1F,

            _ => addr,
        }
    }

    fn reload_vertical_scroll_bits(&mut self) {
        if self.rendering_enabled() {
            let bits = PIXEL_Y_BITS | NAMETABLE_Y_BITS | TILE_Y_BITS;
            self.update_vram_addr((self.vram_addr & !bits) | (self.ppu_addr & bits));
        }
    }

    fn reload_horizontal_scroll_bits(&mut self) {
        if self.rendering_enabled() {
            let bits = NAMETABLE_X_BITS | TILE_X_BITS;
            self.update_vram_addr((self.vram_addr & !bits) | (self.ppu_addr & bits));
        }
    }

    fn set_vblank_status(&mut self) {
        if !self.suppress_next_nmi {
            self.set_ppu_status(self.ppu_status | 0b1000_0000);
        }
        self.suppress_next_nmi = false;
    }

    fn clear_vblank_status(&mut self) {
        self.set_ppu_status(self.ppu_status & 0b0111_1111);
    }

    fn set_sprite_zero_hit(&mut self) {
        self.set_ppu_status(self.ppu_status | 0b0100_0000);
    }

    fn clear_sprite_zero_hit(&mut self) {
        self.set_ppu_status(self.ppu_status & 0b1011_1111);
    }

    fn set_sprite_overflow(&mut self) {
        self.set_ppu_status(self.ppu_status | 0b0010_0000);
    }

    fn clear_sprite_overflow(&mut self) {
        self.set_ppu_status(self.ppu_status & 0b1101_1111);
    }

    fn set_ppu_status(&mut self, val: u8) {
        if val.has_bits(0x80) && !self.ppu_status.has_bits(0x80) && self.ppu_ctrl.has_bits(0x80) {
            self.nmi_triggered = true;
        }

        self.ppu_status = val;
    }

    fn increment_scroll_x(&mut self) {
        if self.rendering_enabled() {
            let mut vram_addr = self.vram_addr;
            if vram_addr.has_bits(TILE_X_BITS) {
                vram_addr &= !TILE_X_BITS;
                vram_addr ^= NAMETABLE_X_BITS;
            } else {
                vram_addr += 1;
            }
            self.update_vram_addr(vram_addr);
        }
    }

    fn increment_scroll_y(&mut self) {
        if self.rendering_enabled() {
            let mut vram_addr = self.vram_addr;
            if vram_addr.has_bits(PIXEL_Y_BITS) {
                // If pixel_y is 7, i.e. bottom of a tile, switch to the next tile.
                // If we're at the bottom of the nametable, switch to the next nametable.

                vram_addr &= !PIXEL_Y_BITS;

                let mut tile_y = (vram_addr & TILE_Y_BITS) >> 5;
                if tile_y == 29 {
                    tile_y = 0;
                    vram_addr ^= NAMETABLE_Y_BITS;
                } else if tile_y == 31 {
                    tile_y = 0;
                } else {
                    tile_y += 1;
                }
                vram_addr = (vram_addr & !TILE_Y_BITS) | (tile_y << 5);
            } else {
                vram_addr += 0b001_0000_0000_0000;
            }
            self.update_vram_addr(vram_addr);
        }
    }

    fn load_bg_shift_registers(&mut self) {
        self.shift_bg_tile_lo = (self.shift_bg_tile_lo & 0xFF00) | self.next_bg_tile_lo as u16;
        self.shift_bg_tile_hi = (self.shift_bg_tile_hi & 0xFF00) | self.next_bg_tile_hi as u16;

        self.shift_bg_attr_lo =
            (self.shift_bg_attr_lo & 0xFF00) | ((self.next_attr_tile & 0b01) as u16 * 0xFF);
        self.shift_bg_attr_hi =
            (self.shift_bg_attr_hi & 0xFF00) | (((self.next_attr_tile & 0b10) >> 1) as u16 * 0xFF);
    }

    fn draw_pixel(&mut self) {
        let x = self.cycle - 1;
        let y = self.scanline;

        let mut bg_opaque = false;

        let mut palette_addr = None;

        if self.background_rendering_enabled() {
            let bit_pos = 0x8000 >> self.pixel_x;
            let pix0 = (self.shift_bg_tile_lo & bit_pos > 0) as u16;
            let pix1 = (self.shift_bg_tile_hi & bit_pos > 0) as u16;
            let pixel_index = (pix1 << 1) | pix0;

            let pal0 = (self.shift_bg_attr_lo & bit_pos > 0) as u16;
            let pal1 = (self.shift_bg_attr_hi & bit_pos > 0) as u16;
            let palette_index = (pal1 << 1) | pal0;

            if pixel_index == 0 {
                if self.bg_rendering_enabled_by_user {
                    palette_addr = Some(0x3F00);
                }
            } else {
                bg_opaque = true;

                if self.bg_rendering_enabled_by_user {
                    palette_addr = Some(0x3F00 | (palette_index << 2) | pixel_index);
                }
            }
        }

        if self.sprite_rendering_enabled() {
            for (_, sprite) in self
                .active_sprites
                .into_iter()
                .filter(|sprite| sprite.active && x >= sprite.x as u16 && x < sprite.x as u16 + 8)
                .enumerate()
            {
                let flip_horizontal = sprite.attrs.has_bits(0b0100_0000);

                let local_x = if flip_horizontal {
                    7 - (x - sprite.x as u16)
                } else {
                    x - sprite.x as u16
                };

                let bit_pos = 0x80 >> local_x;
                let pix0 = sprite.tile_lo.has_bits(bit_pos) as u16;
                let pix1 = sprite.tile_hi.has_bits(bit_pos) as u16;
                let pixel_index = (pix1 << 1) | pix0;

                if pixel_index != 0 {
                    let palette_index = sprite.attrs as u16 & 0b11;

                    let behind_background = sprite.attrs.has_bits(0b0010_0000);

                    if bg_opaque && sprite.idx == 0 {
                        self.set_sprite_zero_hit();
                    }

                    if !behind_background || !bg_opaque {
                        if self.sprite_rendering_enabled_by_user {
                            palette_addr = Some(0x3F10 | (palette_index << 2) | pixel_index);
                        }
                        break;
                    }
                }
            }
        }

        let display_idx = (y * 256 + x) as usize * 4;

        if !self.bg_rendering_enabled_by_user && display_idx < self.display.len() - 4 {
            self.display[display_idx.saturating_add(4)..][..=3].copy_from_slice(&[0, 0, 0, 255]);
        }

        if let Some(addr) = palette_addr {
            let color = DEFAULT_PALETTE[self.read_mem_u8(addr) as usize & 0x3F];
            self.display[display_idx..][..=3].copy_from_slice(&[color.0, color.1, color.2, 255]);
        }
    }

    fn load_nametable_byte(&mut self) {
        self.next_nt_tile = self.read_mem_u8(0x2000 | (self.vram_addr & 0x0FFF));
    }

    fn load_attribute_table_byte(&mut self) {
        let tile_x = self.vram_addr & TILE_X_BITS;
        let tile_y = (self.vram_addr & TILE_Y_BITS) >> 5;

        let addr =
            0x23C0 | (self.vram_addr & NAMETABLE_BITS) | ((tile_y >> 2) << 3) | (tile_x >> 2);

        self.next_attr_tile = self.read_mem_u8(addr);
        self.next_attr_tile >>= (((tile_x & 0b10) >> 1) | (tile_y & 0b10)) << 1;
        self.next_attr_tile &= 0b11;
    }

    fn load_low_bg_tile_byte(&mut self) {
        let bg_tile_addr = self.background_pattern_table_address()
            + ((self.next_nt_tile as u16) << 4)
            + ((self.vram_addr & PIXEL_Y_BITS) >> 12);

        self.next_bg_tile_lo = self.read_mem_u8(bg_tile_addr);
    }

    fn load_high_bg_tile_byte(&mut self) {
        let bg_tile_addr = self.background_pattern_table_address()
            + ((self.next_nt_tile as u16) << 4)
            + ((self.vram_addr & PIXEL_Y_BITS) >> 12)
            + 8;
        self.next_bg_tile_hi = self.read_mem_u8(bg_tile_addr);
    }

    fn advance_bg_shifters(&mut self) {
        if self.background_rendering_enabled() {
            self.shift_bg_tile_lo <<= 1;
            self.shift_bg_tile_hi <<= 1;
            self.shift_bg_attr_lo <<= 1;
            self.shift_bg_attr_hi <<= 1;
        }
    }

    fn load_low_sprite_tile_byte(&mut self) {
        let sprite_idx = (self.cycle as usize - 261) >> 3;
        if self.active_sprites[sprite_idx].active {
            self.active_sprites[sprite_idx].tile_lo =
                self.read_mem_u8(self.sprite_addr(sprite_idx));
        }
    }

    fn load_high_sprite_tile_byte(&mut self) {
        let sprite_idx = (self.cycle as usize - 261) >> 3;
        if self.active_sprites[sprite_idx].active {
            self.active_sprites[sprite_idx].tile_hi =
                self.read_mem_u8(self.sprite_addr(sprite_idx) + 8);
        }
    }

    fn sprite_addr(&self, i: usize) -> u16 {
        let sprite = self.active_sprites[i];

        let next_y = self.scanline + 1;

        let flip_vertical = sprite.attrs.has_bits(0x80);

        let mut local_y = (next_y - sprite.y as u16) & 7;

        if flip_vertical {
            local_y = 7 - local_y;
        }

        let use_large_sprites = self.use_large_sprites();

        let mut sprite_tile = if use_large_sprites {
            sprite.tile_idx as u16 & !1
        } else {
            sprite.tile_idx as u16
        };

        if use_large_sprites
            && ((next_y - sprite.y as u16 > 7 && !flip_vertical)
                || (next_y - sprite.y as u16 <= 7 && flip_vertical))
        {
            sprite_tile += 1;
        }

        let pattern_table = if use_large_sprites {
            (sprite.tile_idx as u16 & 1) << 12
        } else {
            self.sprite_pattern_table_address()
        };

        pattern_table + (sprite_tile << 4) + local_y
    }
}
