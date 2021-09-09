use std::{collections::{BTreeMap, HashMap}, convert::TryInto};

#[derive(Debug, Default)]
pub struct Symbols {
    pub address_to_label: HashMap<u32, Vec<String>>,
    pub label_to_address: HashMap<String, u32>,
}

pub fn read_symbols(input: &[u8]) -> Symbols {
    if input[..3] == b"MND"[..] {
        read_asm68k_symbols(input)
    } else if input[..12] == b"Segment CODE"[..] {
        read_as_symbols(input)
    } else {
        read_nm_symbols(input)
    }
}

fn read_asm68k_symbols(input: &[u8]) -> Symbols {
    let mut address_to_label: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let mut label_to_address: HashMap<String, u32> = HashMap::new();
    let mut i = 8; // skip header
    while i < input.len() {
        let address = u32::from_le_bytes(input[i..i+4].try_into().unwrap());
        i += 4;
        let label_type = input[i];
        i += 1;
        let label_len = input[i] as usize;
        i += 1;
        let label = match label_type {
            2 => String::from_utf8_lossy(&input[i..i+label_len]).to_string(), // global label
            6 => { // local label
                let local_label = String::from_utf8_lossy(&input[i..i+label_len]);
                // local labels are located after the global ones in the symbol file so we should already have seen all the parents by now
                let (_parent_addr, parent_label) = address_to_label.range(..address).next_back().unwrap_or_else(|| panic!("Got local label {} without a parent", local_label));
                let mut combined_label = parent_label.last().unwrap().clone();
                combined_label.push_str(&local_label);
                combined_label
            }
            x => panic!("Unknown label type: {} for {}", x, String::from_utf8_lossy(&input[i..i+label_len])),
        };
        i += label_len;
        address_to_label.entry(address).or_default().push(label.clone());
        label_to_address.insert(label, address);
    }
    Symbols {
        address_to_label: address_to_label.into_iter().collect(),
        label_to_address
    }
}

fn read_as_symbols(input: &[u8]) -> Symbols {
    let mut address_to_symbols: HashMap<u32, Vec<String>> = HashMap::new();
    let mut symbol_to_address: HashMap<String, u32> = HashMap::new();
    let input = String::from_utf8_lossy(input);
    let (_, input_symbols) = input.split_once("Symbols in Segment").expect("Error parsing as symbols");
    for line in input_symbols.lines().skip(1) {
        if line.is_empty() {
            continue;
        }
        let mut elm_iter = line.split_ascii_whitespace();
        let symbol_name = elm_iter.next().expect("Error parsing as symbols");
        let symbol_type = elm_iter.next().expect("Error parsing as symbols");
        if symbol_type != "Int" {
            continue;
        }
        let symbol_addr = elm_iter.next().expect("Error parsing as symbols");
        if let Ok(address) = u64::from_str_radix(symbol_addr, 16) {
            let address = address as u32;
            address_to_symbols.entry(address).or_default().push(symbol_name.to_string());
            symbol_to_address.insert(symbol_name.to_string(), address);
        }
        
    }
    Symbols {
        address_to_label: address_to_symbols,
        label_to_address: symbol_to_address
    }
}

fn read_nm_symbols(input: &[u8]) -> Symbols {
    let mut address_to_symbols: HashMap<u32, Vec<String>> = HashMap::new();
    let mut symbol_to_address: HashMap<String, u32> = HashMap::new();
    let input = String::from_utf8_lossy(input);
    for line in input.split('\n') {
        let elms: Vec<_> = line.split_ascii_whitespace().collect();
        if elms.len() == 3 {
            if let Ok(address) = u32::from_str_radix(elms[0], 16) {
                let label = elms[2];
                address_to_symbols.entry(address).or_default().push(label.to_string());
                symbol_to_address.insert(label.to_string(), address);
            }
        }
    }
    Symbols {
        address_to_label: address_to_symbols,
        label_to_address: symbol_to_address
    }
}
