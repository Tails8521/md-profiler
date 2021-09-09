use std::{collections::{HashMap, HashSet}, fs::File, io::{BufWriter, Write}};

use crate::profiling::{TraceEvent, cycle_to_us};

#[derive(Debug)]
struct IntervalInfo {
    name: String,
    tid: u32,
    reached_at: Option<u64>,
}

#[derive(Debug, Default)]
pub struct Intervals {
    intervals_info: Vec<IntervalInfo>,
    starts: HashMap<u32, Vec<usize>>,
    ends: HashMap<u32, Vec<usize>>,
}

impl Intervals {
    pub fn reach(&mut self, pc: u32, trace_events: &mut Vec<TraceEvent>, cycle: u64, mclk: f64) {
        for &interval_info_index in self.ends.get(&pc).unwrap_or(&vec![]) {
            let interval_info = &mut self.intervals_info[interval_info_index];
            if let Some(reached_at) = interval_info.reached_at {
                let trace_event = TraceEvent {
                    name: interval_info.name.clone().into(),
                    ph: 'X',
                    ts: cycle_to_us(reached_at, mclk),
                    dur: cycle_to_us(cycle - reached_at, mclk),
                    pid: 0,
                    tid: interval_info.tid,
                    args: None,
                    s: None,
                };
                trace_events.push(trace_event);
                interval_info.reached_at = None;
            }
        }
        for &interval_info_index in self.starts.get(&pc).unwrap_or(&vec![]) {
            let interval_info = &mut self.intervals_info[interval_info_index];
            if interval_info.reached_at.is_none() {
                interval_info.reached_at = Some(cycle);
            }
        }
    }

    pub fn write_to_file(&self, output: &mut File) {
        let addresses: HashSet<_> = self.starts.keys().copied().chain(self.ends.keys().copied()).collect();
        let mut buf_writer = BufWriter::new(output);
        for address in addresses {
            buf_writer.write_all(&address.to_ne_bytes()).unwrap();
        }
    }
}

fn read_interval_elm(input: &str, symbols: &HashMap<String, u32>) -> u32 {
    if let Some(&address) = symbols.get(input) {
        return address;
    }
    if let Ok(address) = u32::from_str_radix(input, 16) {
        return address;
    }
    panic!("{} not found in the symbol files", input);
}

pub fn read_intervals(input: &[u8], symbols: &HashMap<String, u32>) -> (Intervals, HashMap<String, u32>) {
    let mut intervals_info = Vec::new();
    let mut starts: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut ends: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut custom_threads: HashMap<String, u32> = HashMap::new();
    let mut current_new_tid = 2;
    let input = String::from_utf8_lossy(input);
    for line in input.split('\n') {
        let line_elms: Vec<_> = line.split(',').collect();
        let interval_index = intervals_info.len();
        if line_elms.len() < 2 {
            continue;
        }
        line_elms[0].trim().split(';').for_each(|elm| {
            let interval_start = read_interval_elm(elm, symbols);
            starts.entry(interval_start).or_default().push(interval_index);
        });
        line_elms[1].trim().split(';').for_each(|elm| {
            let interval_end = read_interval_elm(elm, symbols);
            ends.entry(interval_end).or_default().push(interval_index);
        });
        let tid = if line_elms.len() >= 4 {
            let custom_thread_name = line_elms[3].trim();
            custom_threads.get(custom_thread_name).copied().unwrap_or_else(|| {
                let tid = current_new_tid;
                current_new_tid += 1;
                custom_threads.insert(custom_thread_name.into(), tid);
                tid
            })
        } else {
            0
        };
        let name = if line_elms.len() >= 3 {
            line_elms[2].trim().to_owned()
        } else {
            line.to_owned()
        };
        intervals_info.push(IntervalInfo {
            name,
            tid,
            reached_at: None,
        });
    }
    (
        Intervals {
            intervals_info,
            starts,
            ends,
        },
        custom_threads
    )
}