#![no_std]
#![no_main]

use core::ops::RangeToInclusive;
use cortex_m::asm::nop;
use cortex_m_rt::entry;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};
/// Change this to disable logging
const LOG_ENABLE: bool = true;
macro_rules! _lg {
    ($fmt:expr) => {
        if LOG_ENABLE {
            rprintln!("{}", concat!(line!(), "\t - \t", $fmt));
        };
    };
    () => {
        ($fmt:expr, $($arg:tt)*) => {
            if LOG_ENABLE {
                rprintln!("{}", format_args!(concat!($fmt, "\n"), $($arg)*));
            };
        };
    };
}

pub trait Addressable {
    const ADDR: usize;
}

pub trait Peripheral {}
pub struct GPIO;
pub trait PortKind {}
impl PortKind for GPIO {}
pub struct PortError(ErrorKind);
impl From<PortError> for ReadError {
    #[inline]
    fn from(value: PortError) -> Self {
        ReadError(value.0)
    }
}
impl From<ReadError> for PortError {
    #[inline]
    fn from(value: ReadError) -> Self {
        PortError(value.0)
    }
}
impl From<PortError> for WriteError {
    #[inline]
    fn from(value: PortError) -> Self {
        WriteError(value.0)
    }
}
impl From<WriteError> for PortError {
    #[inline]
    fn from(value: WriteError) -> Self {
        PortError(value.0)
    }
}
pub trait Port: Addressable {
    const RANGE: RangeToInclusive<u8>;
    #[inline]
    fn is_valid(pin_mask: usize) -> bool {
        if pin_mask < Self::RANGE.end as usize {
            true
        } else {
            false
        }
    }
}

// pub trait Offset {
//     const VALUE: usize;
// }

pub enum Access {
    R,
    W,
    RW,
}

pub trait Register {
    type Port: Port;
    const OFFSET: usize;
}
impl<R: Register> Addressable for R {
    const ADDR: usize = R::Port::ADDR + R::OFFSET;
}
pub enum ErrorKind {
    BadIndex,
}
impl core::fmt::Debug for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadIndex => f.write_str("Index out of bounds"),
        }
    }
}

pub struct WriteError(ErrorKind);
impl core::fmt::Debug for WriteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("Write error: {:?}", self.0))
    }
}
impl From<WriteError> for ReadError {
    #[inline]
    fn from(value: WriteError) -> Self {
        ReadError(value.0)
    }
}
pub trait Write: Register + Addressable {
    #[inline]
    fn write(mask: usize, value: Pinstate) -> Result<(), WriteError> {
        if !Self::Port::is_valid(mask) {
            rprintln!("[Write::write] invalid pinmask {:#x}", mask);
            return Err(WriteError(ErrorKind::BadIndex));
        };

        let pin_id = 1 << mask;

        let address = Self::ADDR as *mut usize;

        let register_value = unsafe { core::ptr::read_volatile(address) };

        let masked_value = register_value & pin_id;

        let is_high = masked_value != 1;

        // If the bits mismatch
        if is_high ^ value.is_high() {
            // Flip only that bit
            let value = (pin_id as usize) ^ register_value;
            rprintln!("{:#x}: {:#x} => {:#x}", address.addr(), masked_value, value);
            unsafe { core::ptr::write_volatile(address, value as usize) };
        }
        Ok(())
    }
}

pub struct ReadError(ErrorKind);
impl core::fmt::Debug for ReadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("Read error: {:?}", self.0))
    }
}
impl From<ReadError> for WriteError {
    #[inline]
    fn from(value: ReadError) -> Self {
        WriteError(value.0)
    }
}
pub trait Read: Register + Addressable {
    #[inline]
    fn read(pin_mask: usize) -> Result<Pinstate, ReadError> {
        if !Self::Port::is_valid(pin_mask) {
            rprintln!("[Read::read] invalid pinmask {:#x}", pin_mask);
            return Err(ReadError(ErrorKind::BadIndex));
        };
        Ok(Self::read_unchecked(pin_mask))
        // let pin_id = 1 << (pin_mask as usize);

        // let address = Self::ADDR as *mut usize;
        // // let mut out: u32;
        // let register_value = unsafe { core::ptr::read_volatile(address) };
        // let masked_value = register_value & pin_id;
        // rprintln!("{:#x}: {:#x}", address.addr(), masked_value);
        // Ok((masked_value as u32).into())
    }
    #[inline]
    fn read_unchecked(pin_mask: usize) -> Pinstate {
        // let pin_id = 1 << (pin_mask as usize);

        // let address = (Self::ADDR + Self::OFFSET) as *mut usize;
        let address = Self::ADDR as *mut usize;
        // rprintln!("{}", Self::Port::NAME);
        // rprintln!("{:#x}", address.addr());
        // rprintln!("{:#x}", pin_mask);
        // let mut out: u32;
        let register_value = unsafe { core::ptr::read_volatile(address) };
        let masked_value = register_value & pin_mask;
        (masked_value as u32).into()
    }
}

pub trait RegisterArray<const COUNT: usize> {
    type Port: Port;
    const OFFSET: usize;
    const ADDRS: [usize; COUNT] = const {
        let mut idx = 0;
        let mut addrs = [Self::Port::ADDR + Self::OFFSET; COUNT];
        while idx < COUNT {
            addrs[idx] += idx * 0x4;
            idx += 1;
        }
        addrs
    };
}

pub trait WriteArray<const COUNT: usize>: RegisterArray<COUNT> {
    #[inline]
    fn write_array(pin_mask: usize, value: u32) -> Result<(), WriteError> {
        // Bits 17 and 16 => SENSE
        // Bits 10, 9, and 8 => DRIVE
        // Bits 3 and 2 => PULL
        // Bit 1 => INPUT
        // Bit 0 => DIR
        let address = Self::ADDRS[pin_mask as usize] as *mut usize;
        rprintln!("[WriteArray::write_array] address {:#x}", address.addr(),);
        unsafe { core::ptr::write_volatile(address, value as usize) };
        Ok(())
    }
}

pub trait ReadArray<const COUNT: usize>: RegisterArray<COUNT> {
    #[inline]
    fn read_array(pin_id: usize) -> Result<u32, ReadError> {
        // if !Self::Port::is_valid(pin_mask) {
        //     return Err(ReadError(ErrorKind::BadIndex));
        // };
        let address = Self::ADDRS[pin_id] as *mut usize;
        rprintln!("[ReadArray::read_array] address {:#x}", address.addr(),);
        Ok(unsafe { core::ptr::read_volatile(Self::ADDRS[pin_id as usize] as *mut usize) as u32 })
    }
    #[inline]
    fn read_array_unchecked(pin_id: usize) -> u32 {
        let address = Self::ADDRS[pin_id] as *mut usize;
        rprintln!("[ReadArray::read_array] address {:#x}", address.addr(),);
        unsafe { core::ptr::read_volatile(Self::ADDRS[pin_id as usize] as *mut usize) as u32 }
    }
}

macro_rules! def_portoffset {
    ($name:ident, $addr:literal, $comment:literal) => {
        #[doc = $comment]
        pub struct $name;
        impl crate::Offset for $name {
            const VALUE: usize = $addr;
        }
    };
}
macro_rules! def_rar_trait {
    ($name:ident, $comment:literal, R) => {
        paste::paste! {
            #[doc = "marker trait for" $comment]
            pub trait $name: ReadArray {}
        }
    };
    ($name:ident, $comment:literal, W) => {
        paste::paste! {
            #[doc = "marker trait for" $comment]
            pub trait $name: WriteArray {}
        }
    };
    ($name:ident, $count:literal, $comment:literal, RW) => {
        paste::paste! {
            #[doc = "marker trait for" $comment]
            pub trait $name: ReadArray<$count> + WriteArray<$count> {}
        }
    };
}
macro_rules! rar {
    ($name:ident, $addr:literal, $count:literal, RW, $comment:literal) => {
        def_rar_trait!($name, $count, $comment, RW);
        // def_portoffset!($name, $addr, $comment);
        paste::paste! {
            #[doc = $comment "bank" 0]
            pub struct [<$name 0>];
            impl $name for [<$name 0>] {}
            impl crate::RegisterArray<$count> for [<$name 0>] {
                type Port = P0;
                const OFFSET: usize = $addr;
            }
            impl crate::ReadArray<$count> for [<$name 0>] {}
            impl crate::WriteArray<$count> for [<$name 0>] {}
            #[doc = $comment 1]
            pub struct [<$name 1>];
            impl $name for [<$name 1>] {}
            impl crate::RegisterArray<$count> for [<$name 1>] {
                type Port = P1;
                const OFFSET: usize = $addr;
            }
            impl crate::ReadArray<$count> for [<$name 1>] {}
            impl crate::WriteArray<$count> for [<$name 1>] {}
        }
    };
    ($name:ident, $addr:literal, $count:literal, $acc:ident, $comment:literal) => {
        def_portoffset!($name, $addr, $comment);
        paste::paste! {
            #[doc = $comment "bank" 0]
            pub struct [<$name 0>];
            impl crate::RegisterArray<P0, $name, $count> for [<$name 0>] {
                const ACCESS: crate::Access = $acc;
            }
            #[doc = $comment 1]
            pub struct [<$name 1>];
            impl crate::RegisterArray<P1, $name, $count> for [<$name 1>] {
                const ACCESS: crate::Access = $acc;
            }
        }
    };
}
micro_macro::reg! {OUT, ReadWrite, 0x504}
micro_macro::reg! {OUTSET, ReadWrite, 0x508}
micro_macro::reg! {OUTCLR, ReadWrite, 0x50C}
micro_macro::reg! {IN, Read, 0x510}
// #[reg(ReadWrite, 0x504)]
// pub struct OUT;
// reg!(OUT, 0x504, RW, "Write GPIO port");
// reg!(OUTSET, 0x508, RW, "Set individual bits in GPIO port");
// reg!(OUTCLR, 0x50C, RW, "Clear individual bits in GPIO port");
// reg!(IN, 0x510, R, "Read GPIO port");

//"Direction of GPIO pins"
micro_macro::reg! {DIR, ReadWrite, 0x514}

//"Set direction of GPIO pins"
micro_macro::reg! {DIRSET, ReadWrite, 0x518 }

//"Clear direction of GPIO pins"
micro_macro::reg! {DIRCLR, ReadWrite, 0x51C}
//"Latch register indicating what GPIO pins that have met the criteria set in the [PIN_CNF[n]]. SENSE registers"
micro_macro::reg! {LATCH, ReadWrite, 0x520 }

//"Select between default DETECT signal behavior and LDETECT mode"
micro_macro::reg! {DETECTMODE, ReadWrite, 0x524}
rar!(PINCNF, 0x700, 32, RW, "Configuration of GPIO pins");
impl core::ops::Index<u32> for PINCNF0 {
    type Output = usize;
    fn index(&self, index: u32) -> &Self::Output {
        &Self::ADDRS[index as usize]
    }
}
// impl core::ops::BitOrAssign<(usize, u32)> for PINCNF0 {
//     fn bitor_assign(&mut self, rhs: (usize, u32)) {
//         Self::write_array(rhs.0, rhs.1).unwrap()
//     }
// }
impl core::ops::Index<u32> for PINCNF1 {
    type Output = usize;
    fn index(&self, index: u32) -> &Self::Output {
        &Self::ADDRS[index as usize]
    }
}
// impl core::ops::BitOrAssign<(usize, u32)> for PINCNF1 {
//     fn bitor_assign(&mut self, rhs: (usize, u32)) {
//         Self::write_array(rhs.0, rhs.1).unwrap()
//     }
// }
#[derive(PartialEq)]
pub enum InputbufferState {
    Connected,
    Disconnected,
}
pub trait Pin {
    const PIN_MASK: usize;
    const PIN_ID: usize;
    type Port: Port;
    type OUT: OUT;
    type OUTSET: OUTSET;
    type OUTCLR: OUTCLR;
    type IN: IN;
    type DIR: DIR;
    type DIRSET: DIRSET;
    type DIRCLR: DIRCLR;
    type LATCH: LATCH;
    type DETECTMODE: DETECTMODE;
    type PINCNF: PINCNF;
    /// Read GPIO OUT register for pin
    fn read_out() -> Result<Pinstate, ReadError> {
        Self::OUT::read(Self::PIN_ID)
    }
    /// Write GPIO OUT register for pin
    fn write_out(value: Pinstate) -> Result<(), WriteError> {
        Self::OUT::write(Self::PIN_MASK, value)
    }
    /// Read GPIO IN register for pin
    fn read_in() -> Result<Pinstate, ReadError> {
        if !Self::Port::is_valid(Self::PIN_ID) {
            return Err(ReadError(ErrorKind::BadIndex));
        }
        // rprintln!("[Pin::read_in] ID {:#x}", Self::PIN_ID);
        // rprintln!("[Pin::read_in] {:#x}", Self::Port::ADDR);
        // rprintln!("[Pin::read_in] {:#x}", Self::IN::ADDR);
        // rprintln!("[Pin::read_in] {:#x}", Self::IN::ADDR);
        Ok(Self::IN::read_unchecked(Self::PIN_ID))
    }
    fn read_dir() -> Result<Pinstate, ReadError> {
        Self::DIR::read(Self::PIN_MASK)
    }
    fn write_dir(value: Pinstate) -> Result<(), WriteError> {
        Self::DIR::write(Self::PIN_MASK, value)
    }
    fn read_pincnf() -> Result<u32, ReadError> {
        if !Self::Port::is_valid(Self::PIN_ID) {
            return Err(ReadError(ErrorKind::BadIndex));
        }
        rprintln!("[Pin::read_pincnf] Trying to read from {:#x}", Self::PIN_ID);
        Ok(Self::PINCNF::read_array_unchecked(Self::PIN_ID))
    }
    fn write_pincnf(value: u32) -> Result<(), WriteError> {
        rprintln!("[Pin::write_pincnf] value {:#x}", value,);
        rprintln!("[Pin::write_pincnf] id {:#x}", Self::PIN_ID);
        Self::PINCNF::write_array(Self::PIN_ID as usize, value)
    }
    fn reset_pincnf() -> Result<(), WriteError> {
        Self::write_pincnf(0x0000)
    }
    fn set_input_buffer_as(state: InputbufferState) -> Result<(), WriteError> {
        let current_config = Self::read_pincnf().unwrap();
        let is_connected = (current_config & 0b0010) == 0;
        if (state == InputbufferState::Connected) ^ is_connected {
            return Self::write_pincnf((current_config & 0b0010) ^ current_config);
        }
        Ok(())
    }
    const CONF_MASK: u32 = 0x0003_070F;
    const DIR_MASK: u32 = !0b0001;
    const INPUT_BUF_MASK: u32 = !0b0010;
    fn input_enable() -> Result<(), WriteError> {
        const EN_IN: u32 = 0b0001;
        let previous = Self::read_pincnf()?;
        let value = previous & Self::DIR_MASK & Self::INPUT_BUF_MASK;
        Self::write_pincnf(value | EN_IN)
    }
    const PULL_MASK: u32 = !0b1100;
    fn pull_up() -> Result<(), WriteError> {
        const PULL_UP: u32 = 0b1100;
        let previous = Self::PINCNF::read_array(Self::PIN_ID)?;
        let value = previous & Self::PULL_MASK;
        Self::write_pincnf(value | PULL_UP)
    }
    fn pull_down() -> Result<(), WriteError> {
        const PULL_DOWN: u32 = 0b0100;
        let previous = Self::PINCNF::read_array(Self::PIN_ID)?;
        let value = previous & Self::PULL_MASK;
        Self::write_pincnf(value | PULL_DOWN)
    }
    fn pull_disable() -> Result<(), WriteError> {
        let previous = Self::PINCNF::read_array(Self::PIN_ID)?;
        let value = previous & Self::PULL_MASK;
        Self::write_pincnf(value)
    }
}
macro_rules! __ {
    ($name:ident, $port_number:literal) => {
        paste::paste! {
            type $name = [<$name $port_number>];
        }
    };
}
macro_rules! __def__ {
    ($port_number:literal) => {
        __!(OUT, $port_number);
        __!(OUTSET, $port_number);
        __!(OUTCLR, $port_number);
        __!(IN, $port_number);
        __!(DIR, $port_number);
        __!(DIRSET, $port_number);
        __!(DIRCLR, $port_number);
        __!(LATCH, $port_number);
        __!(DETECTMODE, $port_number);
        __!(PINCNF, $port_number);
    };
}
macro_rules! def_pin {
    (0, $($pin_number:literal),+ $(,)?) => {
        // $($n, $name)*
        $(paste::paste! {
            pub struct [<P 0 $pin_number>];
            impl crate::Pin for [<P 0 $pin_number>] {
                const PIN_ID: usize = $pin_number;
                const PIN_MASK: usize = const { (1usize << $pin_number as usize) >> 1 };
                // const OUT: *mut usize = [<OUT $port_number>]::REG_ADDR as *mut usize;
                type Port = P0;
                __def__!(0);
            }
            impl [<P 0 $pin_number>] {
                fn hello() {
                    rprintln!("this is port 0");
                }
            }
        })*
    };
    ($port_number:literal, $($pin_number:literal),+ $(,)?) => {
        // $($n, $name)*
        $(paste::paste! {
            pub struct [<P $port_number $pin_number>];
            impl crate::Pin for [<P $port_number $pin_number>] {
                const PIN_ID: usize = $pin_number;
                const PIN_MASK: usize = const { (1usize << $pin_number as usize) >> 1 };
                // const OUT: *mut usize = [<OUT $port_number>]::REG_ADDR as *mut usize;
                type Port = [<P $port_number>];
                __!(OUT, $port_number);
                __!(OUTSET, $port_number);
                __!(OUTCLR, $port_number);
                __!(IN, $port_number);
                __!(DIR, $port_number);
                __!(DIRSET, $port_number);
                __!(DIRCLR, $port_number);
                __!(LATCH, $port_number);
                __!(DETECTMODE, $port_number);
                __!(PINCNF, $port_number);
            }
        })*
    };
}
use micro_macro::{address, port};
/// General purpose input and output port
/// P0.00 to P0.31 implemented
#[address(0x5000_0000)]
#[port(..=31)]
pub struct P0;
// impl Port for P0 {
//     const RANGE: RangeToInclusive<u8> = ..=31;
//     const NAME: &'static str = "p0";
// }
def_pin!(
    0, 00, 01, 02, 03, 04, 05, 06, 07, 08, 09, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
    23, 24, 25, 26, 27, 28, 29, 30, 31
);
/// General purpose input and output port
/// P1.00 to P1.09 implemented
pub struct P1;
impl Addressable for P1 {
    const ADDR: usize = 0x5000_0300;
}
impl Port for P1 {
    const RANGE: RangeToInclusive<u8> = ..=9;
}
def_pin!(1, 00, 01, 02, 03, 04, 05, 06, 07, 08, 09);

#[derive(PartialEq)]
#[repr(u8)]
pub enum Pinstate {
    Low,
    High,
}
impl Pinstate {
    fn is_high(&self) -> bool {
        self == &Self::High
    }
}
impl From<Pinstate> for bool {
    fn from(value: Pinstate) -> Self {
        value == Pinstate::High
    }
}
impl From<Pinstate> for u8 {
    fn from(value: Pinstate) -> Self {
        unsafe { core::mem::transmute::<Pinstate, u8>(value) }
    }
}
impl From<Pinstate> for u32 {
    fn from(value: Pinstate) -> Self {
        let intermediate: u32 = value.into();
        intermediate
    }
}
impl From<u32> for Pinstate {
    fn from(value: u32) -> Self {
        match value {
            0 => Pinstate::Low,
            _ => Pinstate::High,
        }
    }
}
// impl Into<u32> for Pinstate {
//     fn into(self) -> u32 {
//         match self {
//             Pinstate::Low => 0,
//             _ => 1,
//         }
//     }
// }
impl core::ops::BitAnd<u32> for Pinstate {
    type Output = u32;
    #[inline]
    fn bitand(self, rhs: u32) -> Self::Output {
        let intermediate: u32 = self.into();
        intermediate & rhs
    }
}
impl core::fmt::Display for Pinstate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Low => f.write_str("Low"),
            Self::High => f.write_str("High"),
        }
    }
}

pub trait Button {}

pub struct ButtonA;
impl ButtonA {
    pub fn is_pressed() -> Result<bool, ReadError> {
        todo!()
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    rprintln!("IM WOKE");
    // Connect input buffer and set as input, pulldown
    // let _ = PINCNF0::write_array(1, 0b0000).unwrap();
    let _ = P000::write_pincnf(0b0000).unwrap();
    rprintln!("{:#x}", P000::PIN_ID);
    let _ = P001::write_pincnf(0b0000).unwrap();
    let _ = P002::write_pincnf(0b0000).unwrap();
    let is_input = DIR0::read(1).unwrap();
    if is_input.is_high() {
        rprintln!("Is input pin, changing that kek");
        let _ = DIR0::write(1, Pinstate::High).unwrap();
    } else {
        rprintln!("Is output pin");
        let _ = DIR0::write(1, Pinstate::Low).unwrap();
    }
    for _ in 0..100_000 {
        nop();
    }
    let is_input = DIR0::read(1).unwrap();
    if is_input.is_high() {
        rprintln!("Is input pin, changing that kek");
        let _ = DIR0::write(1, Pinstate::High).unwrap();
    } else {
        rprintln!("Is output pin");
        let _ = DIR0::write(1, Pinstate::Low).unwrap();
    }
    rprintln!("Trying for P014");
    let bs = P014::read_in().unwrap();
    rprintln!("{}", bs);
    // let value = IN0::read(1).unwrap();
    // rprintln!("pinstate {}", value);
    // Configure as output
    // let _ = P103::write_pincnf(0b0000);
    // for (idx, addr) in <P023 as Pin>::PINCNF::ADDRS.iter().enumerate() {
    //     rprintln!("P023 {} => {:#x}", idx, addr);
    // }
    // let _ = P023::write_pincnf(0b0000).unwrap();
    // let cnf = P023::read_pincnf().unwrap();
    // rprintln!("Got config {:#x}", cnf);
    let cnf = P014::read_pincnf().unwrap();
    rprintln!("Got config {:#x}", cnf);
    P014::input_enable().unwrap();
    let cnf = P014::read_pincnf().unwrap();
    rprintln!("Got config {:#x}", cnf);
    loop {
        _lg!("Soulja boy tell em");
        rprintln!("{}", P014::read_out().unwrap());
        _lg!("Pulling up");
        let _ = P002::pull_up().unwrap();
        for _ in 0..1_000_000 {
            nop();
        }
        rprintln!("Pulling down");
        let _ = P002::pull_down().unwrap();
        for _ in 0..1_000_000 {
            nop();
        }
    }
}
