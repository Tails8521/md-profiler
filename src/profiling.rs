use std::{borrow::{Borrow, Cow}, collections::HashMap, convert::TryInto, fs::File, io::BufWriter, time::Instant};
use serde::Serialize;

use crate::{intervals::Intervals, symbols::Symbols};

const MDP_VERSION: u8 = 1;

const PROFILER_PACKET_SUBROUTINE_ENTER: u8 =  0;
const PROFILER_PACKET_SUBROUTINE_EXIT: u8 =   1;
const PROFILER_PACKET_INTERRUPT_ENTER: u8 =   2;
const PROFILER_PACKET_INTERRUPT_EXIT: u8 =    3;
const PROFILER_PACKET_HINT: u8 =              4;
const PROFILER_PACKET_VINT: u8 =              5;
const PROFILER_PACKET_ADJUST_CYCLES: u8 =     6;
const PROFILER_PACKET_MANUAL_BREAKPOINT: u8 = 7;

#[derive(Debug)]
pub struct ProfilingPacket {
    pub cycle: u64,
    pub stack_pointer: u32,
    pub inner: ProfilingPacketInner,
}

#[derive(Debug)]
pub enum ProfilingPacketInner {
    SubroutineEnter { target_subroutine: u32 },
    SubroutineExit,
    InterruptEnter { target_interrupt: u32 },
    InterruptExit,
    HInt,
    VInt,
    ManualBreakpoint { pc: u32 },
}

#[derive(Debug, Serialize)]
pub struct TraceEventArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_index: Option<u32>
}

#[derive(Debug, Serialize)]
pub struct TraceEvent<'a> {
    pub name: Cow<'a, str>,
    pub ph: char,
    pub ts: f64,
    pub dur: f64,
    pub pid: u32,
    pub tid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<TraceEventArgs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s: Option<char>
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilingJson<'a> {
    trace_events: Vec<TraceEvent<'a>>,
    display_time_unit: &'a str,
}

pub struct ParsedProfilingFile {
    pub packets: Vec<ProfilingPacket>,
    pub mclk: f64,
    pub m68k_divider: u64,
}

pub fn cycle_to_us(cycle: u64, mclk: f64) -> f64 {
    cycle as f64 / mclk * 1_000_000.0
}

pub fn read_profiling_file(input: &[u8]) -> ParsedProfilingFile {
    let mut packets = Vec::new();
    let mut cycle_offset = 0;
    let version = input[3];
    if version != MDP_VERSION {
        eprintln!("Warning: this file is using mdp file format version {} but this application is using version {}", version, MDP_VERSION);
    }
    let mclk = u32::from_ne_bytes(input[4..8].try_into().unwrap()) as f64;
    let m68k_divider = u32::from_ne_bytes(input[8..12].try_into().unwrap()) as u64;
    // advance past the header
    let mut i = 256;
    while i < input.len() {
        let packet_type = input[i];
        i += 1;
        let cycle32 = u32::from_ne_bytes(input[i..i+4].try_into().unwrap());
        i += 4;
        let cycle = cycle_offset + cycle32 as u64;
        let stack_pointer = u32::from_ne_bytes(input[i..i+4].try_into().unwrap());
        i += 4;
        let inner = match packet_type {
            PROFILER_PACKET_SUBROUTINE_ENTER => {
                let target_subroutine = u32::from_ne_bytes(input[i..i+4].try_into().unwrap());
                i += 4;
                ProfilingPacketInner::SubroutineEnter {
                    target_subroutine
                }
            },
            PROFILER_PACKET_SUBROUTINE_EXIT => ProfilingPacketInner::SubroutineExit,
            PROFILER_PACKET_INTERRUPT_ENTER => {
                let target_interrupt = u32::from_ne_bytes(input[i..i+4].try_into().unwrap());
                i += 4;
                ProfilingPacketInner::InterruptEnter {
                    target_interrupt
                }
            },
            PROFILER_PACKET_INTERRUPT_EXIT => ProfilingPacketInner::InterruptExit,
            PROFILER_PACKET_HINT => ProfilingPacketInner::HInt,
            PROFILER_PACKET_VINT => ProfilingPacketInner::VInt,
            PROFILER_PACKET_ADJUST_CYCLES => {
                cycle_offset += cycle32 as u64;
                continue;
            },
            PROFILER_PACKET_MANUAL_BREAKPOINT => {
                let pc = u32::from_ne_bytes(input[i..i+4].try_into().unwrap());
                i += 4;
                ProfilingPacketInner::ManualBreakpoint {
                    pc
                }
            }
            x => panic!("Unknown packet type: {}", x)
        };
        let packet = ProfilingPacket {
            cycle,
            stack_pointer,
            inner
        };
        packets.push(packet);
    }
    ParsedProfilingFile {
        packets,
        mclk,
        m68k_divider
    }
}

pub fn generate_profiling_json(mut output: &mut File, input: &ParsedProfilingFile, symbols: &Symbols, intervals: &mut Intervals, custom_threads: HashMap<String, u32>) {
    let mut trace_events = vec![
        TraceEvent {
            name: "process_name".into(),
            ph: 'M',
            ts: 0.0,
            dur: 0.0,
            pid: 0,
            tid: 0,
            args: Some(TraceEventArgs {
                name: Some("M68000".into()),
                sort_index: None,
            }),
            s: None,
        },
        TraceEvent {
            name: "thread_name".into(),
            ph: 'M',
            ts: 0.0,
            dur: 0.0,
            pid: 0,
            tid: 0,
            args: Some(TraceEventArgs {
                name: Some("Main thread".into()),
                sort_index: None,
            }),
            s: None,
        },
        TraceEvent {
            name: "thread_name".into(),
            ph: 'M',
            ts: 0.0,
            dur: 0.0,
            pid: 0,
            tid: 1,
            args: Some(TraceEventArgs {
                name: Some("Interrupts".into()),
                sort_index: None,
            }),
            s: None,
        },
        TraceEvent {
            name: "thread_sort_index".into(),
            ph: 'M',
            ts: 0.0,
            dur: 0.0,
            pid: 0,
            tid: 0,
            args: Some(TraceEventArgs {
                name: None,
                sort_index: Some(0)
            }),
            s: None,
        },
        TraceEvent {
            name: "thread_sort_index".into(),
            ph: 'M',
            ts: 0.0,
            dur: 0.0,
            pid: 0,
            tid: 1,
            args: Some(TraceEventArgs {
                name: None,
                sort_index: Some(1)
            }),
            s: None,
        },
    ];
    for (name, tid) in custom_threads {
        trace_events.push(
            TraceEvent {
                name: "thread_name".into(),
                ph: 'M',
                ts: 0.0,
                dur: 0.0,
                pid: 0,
                tid,
                args: Some(TraceEventArgs {
                    name: Some(name),
                    sort_index: None,
                }),
                s: None,
            },
        );
        trace_events.push(
            TraceEvent {
                name: "thread_sort_index".into(),
                ph: 'M',
                ts: 0.0,
                dur: 0.0,
                pid: 0,
                tid,
                args: Some(TraceEventArgs {
                    name: None,
                    sort_index: Some(tid)
                }),
                s: None,
            }
        )
    }
    let last_cycle = input.packets.last().unwrap().cycle + 1;
    let mut tid = 0;
    let instant = Instant::now();
    for (i, packet) in input.packets.iter().enumerate() {
        match packet.inner {
            ProfilingPacketInner::SubroutineEnter { target_subroutine } => {
                let mut end_cycle = last_cycle;
                for matching_packet in &input.packets[i+1..] {
                    if let ProfilingPacketInner::SubroutineExit = matching_packet.inner {
                        // + 4 because the RTS hasn't been executed yet so the PC has yet to be popped off the stack
                        if matching_packet.stack_pointer + 4 >= packet.stack_pointer {
                            end_cycle = matching_packet.cycle;
                            break;
                        }
                    }
                }
                let name = match symbols.address_to_label.get(&target_subroutine) {
                    Some(labels) => Cow::Borrowed(labels.last().unwrap().borrow()),
                    None => Cow::Owned(format!("{:#x}", target_subroutine)),
                };
                let trace_event = TraceEvent {
                    name,
                    ph: 'X',
                    ts: cycle_to_us(packet.cycle, input.mclk),
                    dur: cycle_to_us(end_cycle - packet.cycle, input.mclk),
                    pid: 0,
                    tid,
                    args: None,
                    s: None,
                };
                trace_events.push(trace_event);
            },
            ProfilingPacketInner::InterruptEnter { target_interrupt} => {
                tid = 1;
                let mut end_cycle = last_cycle;
                for matching_packet in &input.packets[i+1..] {
                    if let ProfilingPacketInner::InterruptExit = matching_packet.inner {
                        end_cycle = matching_packet.cycle;
                        break;
                    }
                }
                let name = match symbols.address_to_label.get(&target_interrupt) {
                    Some(labels) => Cow::Borrowed(labels.last().unwrap().borrow()),
                    None => Cow::Owned(format!("{:#x}", target_interrupt)),
                };
                let trace_event = TraceEvent {
                    name,
                    ph: 'X',
                    ts: cycle_to_us(packet.cycle, input.mclk),
                    dur: cycle_to_us(end_cycle - packet.cycle, input.mclk),
                    pid: 0,
                    tid,
                    args: None,
                    s: None,
                };
                trace_events.push(trace_event);
            },
            ProfilingPacketInner::InterruptExit => {
                tid = 0;
            },
            // ProfilingPacketInner::HInt => {
            //     let trace_event = TraceEvent {
            //         name: "HInt".into(),
            //         ph: 'i',
            //         ts: cycle_to_us(packet.cycle, input.mclk),
            //         dur: 0.0,
            //         pid: 0,
            //         tid: 1,
            //         args: None,
            //         s: Some('g'),
            //     };
            //     trace_events.push(trace_event);
            // },
            ProfilingPacketInner::VInt => {
                let trace_event = TraceEvent {
                    name: "VInt".into(),
                    ph: 'i',
                    ts: cycle_to_us(packet.cycle, input.mclk),
                    dur: 0.0,
                    pid: 0,
                    tid: 1,
                    args: None,
                    s: Some('g'),
                };
                trace_events.push(trace_event);
            },
            ProfilingPacketInner::ManualBreakpoint { pc } => {
                intervals.reach(pc, &mut trace_events, packet.cycle, input.mclk);
            }

            _ => {},
        }
    }
    let elapsed = instant.elapsed();
    println!("Generated {} output events in {} ms", trace_events.len(), elapsed.as_micros() as f64 / 1000.0);
    let instant = Instant::now();
    serde_json::ser::to_writer(BufWriter::new(&mut output), &ProfilingJson {
        trace_events,
        display_time_unit: "ms",
    }).expect("Error writing json file");
    let elapsed = instant.elapsed();
    println!("Wrote {} MB of json in {} ms", output.metadata().unwrap().len() / 1_000_000, elapsed.as_micros() as f64 / 1000.0);
}