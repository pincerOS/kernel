#![no_std]

extern crate core;

mod local;

pub use local::BufferHandle;

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ConnRequest {
    pub width: u16,
    pub height: u16,
    pub min_width: u16,
    pub min_height: u16,
    pub max_width: u16,
    pub max_height: u16,
}

unsafe impl bytemuck::Zeroable for ConnRequest {}
unsafe impl bytemuck::Pod for ConnRequest {}

#[allow(non_camel_case_types)]
type au8 = AtomicU8;
#[allow(non_camel_case_types)]
type au32 = AtomicU32;

#[repr(C)]
#[derive(Copy, Clone, PartialEq)]
pub struct SemDescriptor(pub u32);

#[repr(C)]
pub struct BufferHeader {
    pub version: au32,
    pub magic: au32,
    pub kill_switch: au32,
    pub last_words: [au8; 32],

    pub meta: GlobalMeta,

    pub client_to_server_queue: EventQueue,
    pub server_to_client_queue: EventQueue,

    pub video_meta: VideoMeta,
    pub term_meta: TermMeta,

    pub present_sem: SemDescriptor,
}

#[repr(C)]
pub struct GlobalMeta {
    pub segment_size: u32,
    // Offset from the start of the buffer to the start of vmem
    pub vmem_offset: u32,
    pub vmem_size: u32,
}

#[repr(C)]
pub struct VideoMeta {
    pub width: u16,
    pub height: u16,
    pub row_stride: u16,
    pub bytes_per_pixel: u8,
    pub bit_layout: u8,

    pub present_ts: u64,
}

#[repr(C)]
pub struct TermMeta {
    pub rows: u16,
    pub cols: u16,
}

#[repr(C)]
#[derive(PartialEq, Eq)]
pub struct EventKind(pub u64);

#[repr(C)]
pub struct Event {
    pub kind: EventKind,
    pub data: [u64; 7],
}

const EVENT_BUF_SIZE: usize = 128;

// TODO: Efficient ringbuffer (cache lines?)
// TODO: frozen event repr?  (ie. bytemuck events?)
// head = index of next unused element slot
// tail = index of oldest element
// empty if head == tail (mod len)
//      ((head - tail).mod(len) == 0)
// full if head == tail - 1 (mod len)
//      ((head - (tail - 1)).mod(len) == 0)
// Wastes one slot, but that's probably fine
#[repr(C)]
pub struct EventQueue {
    pub head: AtomicU32,
    pub elems: core::cell::UnsafeCell<[MaybeUninit<Event>; EVENT_BUF_SIZE]>,
    pub tail: AtomicU32,
}

const _: () = assert!(EVENT_BUF_SIZE.next_power_of_two() == EVENT_BUF_SIZE);

// SPSC ringbuffer; each side is expected to have a lock on their
// local copy.  The shared memory is untrusted, so this must not
// trust the other end for safety or correctness.
impl EventQueue {
    pub fn new() -> Self {
        EventQueue {
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            elems: core::cell::UnsafeCell::new([const { MaybeUninit::uninit() }; EVENT_BUF_SIZE]),
        }
    }

    fn empty(head: u32, tail: u32) -> bool {
        head == tail
    }
    fn full(head: u32, tail: u32) -> bool {
        head == tail.wrapping_add(EVENT_BUF_SIZE as u32)
    }
    pub fn try_send(&self, event: Event) -> Result<(), Event> {
        let cur_head = self.head.load(Ordering::Relaxed);
        let cur_tail = self.tail.load(Ordering::SeqCst);

        if Self::full(cur_head, cur_tail) {
            return Err(event);
        }

        let head_idx = (cur_head as usize).rem_euclid(EVENT_BUF_SIZE);
        let elems = self.elems.get().cast::<Event>();
        let target = elems.wrapping_add(head_idx);

        // Safety: cur_head is always within range for the elems array.
        // This does not create an intermediate reference, and just writes
        // to a value within an UnsafeCell.  If one side is buggy or malicious,
        // this can only corrupt the data/queue state, and cannot influence
        // the control flow on this side of the channel.
        //
        // Correctness: There is a single producer for this queue, so
        // we can write data to the unused region without synchronization
        // concerns.  The tail pointer will only shrink the region, and
        // not grow backwards; as this target is outside of the head..tail
        // region, it is unused.
        unsafe {
            target.write_volatile(event);
        }

        // After writing the event, fence and increment the head to make
        // the event visible to the consumer.
        // TODO: is SeqCst enough of a fence for this?
        self.head.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }

    pub fn try_recv(&self) -> Option<Event> {
        let cur_head = self.head.load(Ordering::SeqCst);
        let cur_tail = self.tail.load(Ordering::Relaxed);
        // TODO: are the above loads enough of a fence?

        if Self::empty(cur_head, cur_tail) {
            return None;
        }

        let tail_idx = (cur_tail as usize).rem_euclid(EVENT_BUF_SIZE);
        let elems = self.elems.get().cast::<MaybeUninit<Event>>();
        let target = elems.wrapping_add(tail_idx);

        // Safety: cur_tail is always within range for the elems array.
        // This does not create an intermediate reference, and just writes
        // to a value within an UnsafeCell.  If one side is buggy or malicious,
        // this can only corrupt the data/queue state, and cannot influence
        // the control flow on this side of the channel.
        //
        // Correctness: There is a single consumer for this queue;
        // events within the range (head, tail] are all initialized and unchanging.
        // The producer will only reuse the slot once the tail has been incremented.
        let event = unsafe { target.read_volatile().assume_init() };

        self.tail.fetch_add(1, Ordering::SeqCst);

        Some(event)
    }

    pub fn try_send_data<E>(&self, data: E) -> Result<(), E>
    where
        E: EventData,
    {
        let event = Event {
            kind: E::KIND,
            data: data.serialize_data(),
        };
        self.try_send(event).map_err(|_| data)
    }
}

pub trait EventData: Sized {
    const KIND: EventKind;
    fn parse(event: &Event) -> Option<Self> {
        if event.kind == Self::KIND {
            Self::parse_data(&event.data)
        } else {
            None
        }
    }
    fn parse_data(data: &[u64; 7]) -> Option<Self>;
    fn serialize_data(&self) -> [u64; 7];
}

impl EventKind {
    pub const UNSET: EventKind = EventKind(0);
    pub const PRESENT: EventKind = EventKind(1);
    pub const INPUT: EventKind = EventKind(2);
    pub const DISCONNECT: EventKind = EventKind(3);
    pub const REQUEST_CLOSE: EventKind = EventKind(5);
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct PresentEvent;

unsafe impl bytemuck::Zeroable for PresentEvent {}
unsafe impl bytemuck::AnyBitPattern for PresentEvent {}

impl EventData for PresentEvent {
    const KIND: EventKind = EventKind::PRESENT;
    fn parse_data(data: &[u64; 7]) -> Option<Self> {
        const _ASSERT: () = assert!(size_of::<PresentEvent>() <= size_of::<[u64; 7]>());
        let bytes = &bytemuck::bytes_of(data)[..size_of::<Self>()];
        Some(*bytemuck::from_bytes(bytes))
    }
    fn serialize_data(&self) -> [u64; 7] {
        [0; 7]
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct InputEvent {
    pub kind: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
}
impl InputEvent {
    pub const KIND_KEY: u32 = 1;
    // data1 = mode (1 = press, 2 = release, 3 = repeat)
    // data2 = scan code
    pub const KIND_MOUSE: u32 = 2;
    // data1 = mode (1 = move, 2 = down, 3 = up)
    // data2 = x position (pixels)
    // data3 = y position (pixels)
    // data4 = button (1 = left, 2 = right, 3 = middle, ...)
    pub const KIND_SCROLL: u32 = 3;
    // data1 = x delta (pixels)
    // data2 = y delta (pixels)
}

unsafe impl bytemuck::Zeroable for InputEvent {}
unsafe impl bytemuck::AnyBitPattern for InputEvent {}
impl EventData for InputEvent {
    const KIND: EventKind = EventKind::INPUT;
    fn parse_data(data: &[u64; 7]) -> Option<Self> {
        const _ASSERT: () = assert!(size_of::<InputEvent>() <= size_of::<[u64; 7]>());
        let bytes = &bytemuck::bytes_of(data)[..size_of::<Self>()];
        Some(*bytemuck::from_bytes(bytes))
    }
    fn serialize_data(&self) -> [u64; 7] {
        let mut out = [0u64; 7];
        // TODO: serialization soundness; needs no uninitialized data,
        // as it could leak secrets
        let data: [u8; size_of::<Self>()] = unsafe { core::mem::transmute(*self) };
        bytemuck::cast_slice_mut(&mut out)[..size_of::<Self>()].copy_from_slice(&data);
        out
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct RequestCloseEvent;

impl EventData for RequestCloseEvent {
    const KIND: EventKind = EventKind::REQUEST_CLOSE;
    fn parse_data(_data: &[u64; 7]) -> Option<Self> {
        Some(RequestCloseEvent)
    }
    fn serialize_data(&self) -> [u64; 7] {
        [0u64; 7]
    }
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub struct ScanCode(pub u32);

macro_rules! define_scancodes {
    ($($name:ident = $num:literal $( ( $char:literal $( / $shift:literal )? ) )?),* $(,)?) => {
        #[allow(unused)]
        impl ScanCode {
            $(pub const $name: ScanCode = ScanCode($num);)*
        }
        #[allow(unused)]
        pub const SCANCODES: [Option<char>; 256] = {
            #[allow(unused_mut)]
            let mut s = [None; 256];
            $($(s[$num] = Some($char);)?)*
            s
        };
        #[allow(unused)]
        pub const SCANCODES_SHIFTED: [Option<char>; 256] = {
            #[allow(unused_mut)]
            let mut s = [None; 256];
            $($($(s[$num] = Some($shift);)?)?)*
            s
        };
    };
}

define_scancodes! {
    KEY0 = 0 ('0' / ')'),
    KEY1 = 1 ('1' / '!'),
    KEY2 = 2 ('2' / '@'),
    KEY3 = 3 ('3' / '#'),
    KEY4 = 4 ('4' / '$'),
    KEY5 = 5 ('5' / '%'),
    KEY6 = 6 ('6' / '^'),
    KEY7 = 7 ('7' / '&'),
    KEY8 = 8 ('8' / '*'),
    KEY9 = 9 ('9' / '('),
    A = 10 ('a' / 'A'),
    B = 11 ('b' / 'B'),
    C = 12 ('c' / 'C'),
    D = 13 ('d' / 'D'),
    E = 14 ('e' / 'E'),
    F = 15 ('f' / 'F'),
    G = 16 ('g' / 'G'),
    H = 17 ('h' / 'H'),
    I = 18 ('i' / 'I'),
    J = 19 ('j' / 'J'),
    K = 20 ('k' / 'K'),
    L = 21 ('l' / 'L'),
    M = 22 ('m' / 'M'),
    N = 23 ('n' / 'N'),
    O = 24 ('o' / 'O'),
    P = 25 ('p' / 'P'),
    Q = 26 ('q' / 'Q'),
    R = 27 ('r' / 'R'),
    S = 28 ('s' / 'S'),
    T = 29 ('t' / 'T'),
    U = 30 ('u' / 'U'),
    V = 31 ('v' / 'V'),
    W = 32 ('w' / 'W'),
    X = 33 ('x' / 'X'),
    Y = 34 ('y' / 'Y'),
    Z = 35 ('z' / 'Z'),
    F1 = 36,
    F2 = 37,
    F3 = 38,
    F4 = 39,
    F5 = 40,
    F6 = 41,
    F7 = 42,
    F8 = 43,
    F9 = 44,
    F10 = 45,
    F11 = 46,
    F12 = 47,
    F13 = 48,
    F14 = 49,
    F15 = 50,
    DOWN = 51,
    LEFT = 52,
    RIGHT = 53,
    UP = 54,
    APOSTROPHE = 55 ('\'' / '"'),
    BACKQUOTE = 56 ('`' / '~'),
    BACKSLASH = 57 ('\\' / '|'),
    COMMA = 58 (',' / '<'),
    EQUAL = 59 ('=' / '+'),
    LEFT_BRACKET = 60 ('[' / '{'),
    MINUS = 61 ('-' / '_'),
    PERIOD = 62 ('.' / '>'),
    RIGHT_BRACKET = 63 (']' / '}'),
    SEMICOLON = 64 (';' / ':'),
    SLASH = 65 ('/' / '?'),
    BACKSPACE = 66,
    DELETE = 67,
    END = 68,
    ENTER = 69 ('\n'),
    ESCAPE = 70,
    HOME = 71,
    INSERT = 72,
    MENU = 73,
    PAGE_DOWN = 74,
    PAGE_UP = 75,
    PAUSE = 76,
    SPACE = 77 (' '),
    TAB = 78 ('\t'),
    NUM_LOCK = 79,
    CAPS_LOCK = 80,
    SCROLL_LOCK = 81,
    LEFT_SHIFT = 82,
    RIGHT_SHIFT = 83,
    LEFT_CTRL = 84,
    RIGHT_CTRL = 85,
    NUM_PAD0 = 86 ('0'),
    NUM_PAD1 = 87 ('1'),
    NUM_PAD2 = 88 ('2'),
    NUM_PAD3 = 89 ('3'),
    NUM_PAD4 = 90 ('4'),
    NUM_PAD5 = 91 ('5'),
    NUM_PAD6 = 92 ('6'),
    NUM_PAD7 = 93 ('7'),
    NUM_PAD8 = 94 ('8'),
    NUM_PAD9 = 95 ('9'),
    NUM_PAD_DOT = 96 ('.'),
    NUM_PAD_SLASH = 97 ('/'),
    NUM_PAD_ASTERISK = 98 ('*'),
    NUM_PAD_MINUS = 99 ('-'),
    NUM_PAD_PLUS = 100 ('+'),
    NUM_PAD_ENTER = 101 ('\n'),
    LEFT_ALT = 102,
    RIGHT_ALT = 103,
    LEFT_SUPER = 104,
    RIGHT_SUPER = 105,
    UNKNOWN = 106,
}

#[cfg(target_arch = "aarch64")]
#[track_caller]
pub fn memcpy128(dst: &mut [u128], src: &[u128]) {
    let len = dst.len();
    assert_eq!(len, src.len());
    assert!(len % 64 == 0);
    unsafe {
        core::arch::asm!(r"
        1:
        ldp {tmp1}, {tmp2}, [{src}, #0]
        stp {tmp1}, {tmp2}, [{dst}, #0]
        ldp {tmp1}, {tmp2}, [{src}, #16]
        stp {tmp1}, {tmp2}, [{dst}, #16]
        ldp {tmp1}, {tmp2}, [{src}, #32]
        stp {tmp1}, {tmp2}, [{dst}, #32]
        ldp {tmp1}, {tmp2}, [{src}, #48]
        stp {tmp1}, {tmp2}, [{dst}, #48]
        add {src}, {src}, #64 // TODO: figure out east way to use index increment
        add {dst}, {dst}, #64
        subs {count}, {count}, #4
        b.hi 1b // if count > 0, loop
        ",
        src = in(reg) src.as_ptr(),
        dst = in(reg) dst.as_mut_ptr(),
        count = in(reg) len,
        tmp1 = out(reg) _, tmp2 = out(reg) _,
        )
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub fn memcpy128(dst: &mut [u128], src: &[u128]) {
    dst.copy_from_slice(src)
}
