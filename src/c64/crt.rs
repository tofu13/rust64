use c64::memory;
use std;
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::str;

use byteorder::{BigEndian, ReadBytesExt};
use enum_primitive::FromPrimitive;

#[derive(Debug)]
pub struct Crt {
    header: Header,
    chips: Vec<Chip>,
}

impl Crt {
    pub fn from_filename(filename: &str) -> Result<Crt, String> {
        let mut file = File::open(filename).map_err(|e| e.to_string())?;

        // Read Header
        let mut signature = [0u8; 16];
        file.read(&mut signature).map_err(|e| e.to_string())?;
        if &signature != b"C64 CARTRIDGE   " {
            return Err("Invalid cartridge signature".to_string());
        }
        let header_len = file.read_u32::<BigEndian>().map_err(|e| e.to_string())?;
        let mut version = [0u8; 2];
        file.read(&mut version).map_err(|e| e.to_string())?;
        let hw_type = file.read_u16::<BigEndian>().map_err(|e| e.to_string())?;
        if hw_type != 0 {
            return Err("Unsupported cartridge type".to_string());
        }
        let exrom = file.read_u8().map_err(|e| e.to_string())?;
        let game = file.read_u8().map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(0x20))
            .map_err(|e| e.to_string())?;
        let mut name = [0u8; 32];
        file.read(&mut name).map_err(|e| e.to_string())?;

        // Read Chips
        file.seek(SeekFrom::Start(header_len as u64))
            .map_err(|e| e.to_string())?;
        let mut chips: Vec<Chip> = Vec::new();
        loop {
            let mut chip_signature = [0u8; 4];
            file.read(&mut chip_signature).map_err(|e| e.to_string())?;
            if &chip_signature != b"CHIP" {
                break;
            }
            let length = file.read_u32::<BigEndian>().map_err(|e| e.to_string())?;
            let chip_type =
                ChipType::from_u16(file.read_u16::<BigEndian>().map_err(|e| e.to_string())?)
                    .ok_or("Invalid chip type".to_string())?;
            let bank_number = file.read_u16::<BigEndian>().map_err(|e| e.to_string())?;
            let load_addr = file.read_u16::<BigEndian>().map_err(|e| e.to_string())?;
            let data_size = file.read_u16::<BigEndian>().map_err(|e| e.to_string())?;
            let mut data: Vec<u8> = vec![0u8; data_size as usize];
            file.read(&mut data).map_err(|e| e.to_string())?;

            chips.push(Chip {
                signature: chip_signature,
                length,
                chip_type,
                bank_number,
                load_addr,
                data_size,
                data,
            });
        }

        Ok(Crt {
            header: Header {
                signature,
                header_len,
                version,
                hw_type,
                exrom,
                game,
                name,
            },
            chips,
        })
    }

    pub fn load_into_memory(&self, mut memory: std::cell::RefMut<memory::Memory>) {
        memory.exrom = self.header.exrom == 1;
        memory.game = self.header.game == 1;
        for chip in self.chips.iter() {
            let base_addr = chip.load_addr;
            for (offset, byte) in chip.data.iter().enumerate() {
                memory.write_byte(base_addr + offset as u16, *byte);
            }
        }
    }
}

struct Header {
    signature: [u8; 16],
    header_len: u32,
    version: [u8; 2],
    hw_type: u16,
    exrom: u8,
    game: u8,
    // 001A-001F RFU
    name: [u8; 32],
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Header {{
    signature: {},
    header_len: {} bytes,
    version: {:x}.{:02x}
    hw_type: {},
    exrom: {},
    game: {},
    name: {}
}}",
            str::from_utf8(&self.signature).unwrap(),
            self.header_len,
            self.version[0],
            self.version[1],
            self.hw_type,
            self.exrom,
            self.game,
            str::from_utf8(&self.name).unwrap()
        )
    }
}

struct Chip {
    signature: [u8; 4],
    length: u32, // header and data combined
    chip_type: ChipType,
    bank_number: u16,
    load_addr: u16,
    data_size: u16,
    data: Vec<u8>,
}

impl fmt::Debug for Chip {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Chip {{
    signature: {},
    length: {} bytes,
    chip_type: {:?},
    bank_number: {},
    load_addr: 0x{:04x},
    data_size: {} bytes,
    data: (not shown)
}}",
            str::from_utf8(&self.signature).unwrap(),
            self.length,
            self.chip_type,
            self.bank_number,
            self.load_addr,
            self.data_size
        )
    }
}

enum_from_primitive! {
    #[derive(Debug, PartialEq)]
    enum ChipType {
        ROM,
        RAM,
        Flash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    // builds the bytes of a minimal, single-chip .crt file matching the
    // layout that Crt::from_filename expects
    fn build_crt_bytes(exrom: u8, game: u8, hw_type: u16, load_addr: u16, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"C64 CARTRIDGE   "); // 16-byte signature
        bytes.extend_from_slice(&64u32.to_be_bytes()); // header_len
        bytes.extend_from_slice(&[0x01, 0x00]); // version
        bytes.extend_from_slice(&hw_type.to_be_bytes());
        bytes.push(exrom);
        bytes.push(game);
        bytes.resize(0x20, 0); // pad up to the name field offset
        bytes.extend_from_slice(&[0u8; 32]); // name
        assert_eq!(bytes.len(), 0x40); // matches header_len above

        bytes.extend_from_slice(b"CHIP");
        let chip_len = (16 + data.len()) as u32; // chip sub-header + data
        bytes.extend_from_slice(&chip_len.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes()); // chip_type = ROM
        bytes.extend_from_slice(&0u16.to_be_bytes()); // bank_number
        bytes.extend_from_slice(&load_addr.to_be_bytes());
        bytes.extend_from_slice(&(data.len() as u16).to_be_bytes());
        bytes.extend_from_slice(data);

        bytes
    }

    fn write_temp_crt(bytes: &[u8]) -> std::path::PathBuf {
        let n = TEMP_FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("rust64_test_crt_{}_{}.crt", std::process::id(), n));
        let mut f = File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        path
    }

    #[test]
    fn from_filename_reads_header_and_loads_chip_data_into_memory() {
        let data = vec![0xA9, 0x01, 0x60];
        let bytes = build_crt_bytes(1, 0, 0, 0x8000, &data);
        let path = write_temp_crt(&bytes);

        let crt = Crt::from_filename(path.to_str().unwrap()).unwrap();

        let mem_shared = memory::Memory::new_shared();
        crt.load_into_memory(mem_shared.borrow_mut());

        let mut mem = mem_shared.borrow_mut();
        assert!(mem.exrom); // header exrom byte was 1
        assert!(!mem.game); // header game byte was 0
        assert_eq!(mem.read_byte(0x8000), 0xA9);
        assert_eq!(mem.read_byte(0x8001), 0x01);
        assert_eq!(mem.read_byte(0x8002), 0x60);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn from_filename_rejects_invalid_signature() {
        let mut bytes = build_crt_bytes(0, 0, 0, 0x8000, &[0x00]);
        bytes[0] = b'X'; // corrupt the signature
        let path = write_temp_crt(&bytes);

        let result = Crt::from_filename(path.to_str().unwrap());

        assert_eq!(result.unwrap_err(), "Invalid cartridge signature");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn from_filename_rejects_unsupported_hardware_type() {
        let bytes = build_crt_bytes(0, 0, 1, 0x8000, &[0x00]); // hw_type = 1
        let path = write_temp_crt(&bytes);

        let result = Crt::from_filename(path.to_str().unwrap());

        assert_eq!(result.unwrap_err(), "Unsupported cartridge type");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn from_filename_returns_error_for_missing_file() {
        let result = Crt::from_filename("/nonexistent/path/does_not_exist.crt");
        assert!(result.is_err());
    }
}
