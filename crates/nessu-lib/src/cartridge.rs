use crate::header::Header;
use crate::mapper::{build_mapper, Mapper, MapperTrait, Mirroring};
use log::debug;

#[derive(Clone)]
pub struct Cartridge {
    header: Header,
    mapper: Mapper,
    valid: bool,
}

impl Default for Cartridge {
    fn default() -> Self {
        let header = Header::default();
        let mapper = build_mapper(&[], &header).unwrap();
        Self {
            header,
            mapper,
            valid: false,
        }
    }
}

impl Cartridge {
    pub fn from_bytes(bytes: &[u8]) -> std::io::Result<Self> {
        let header = Header::read_from_slice(bytes)?;

        debug!("{:?}", header);

        let mapper = build_mapper(bytes, &header)?;

        Ok(Self {
            header,
            mapper,
            valid: true,
        })
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn mirroring(&self) -> Mirroring {
        self.mapper.mirroring().unwrap_or(self.header.mirroring)
    }

    pub fn cpu_read_u8(&mut self, addr: usize) -> u8 {
        self.mapper.cpu_read_u8(addr)
    }

    pub fn cpu_write_u8(&mut self, addr: usize, val: u8, cycle: u128) {
        self.mapper.cpu_write_u8(addr, val, cycle);
    }

    pub fn ppu_read_u8(&mut self, addr: usize) -> Option<u8> {
        self.mapper.ppu_read_u8(addr)
    }

    pub fn ppu_write_u8(&mut self, addr: usize, val: u8) -> bool {
        self.mapper.ppu_write_u8(addr, val)
    }

    pub fn irq_triggered(&mut self) -> bool {
        self.mapper.irq_triggered()
    }

    pub fn clock_irq(&mut self) {
        self.mapper.clock_irq();
    }
}
