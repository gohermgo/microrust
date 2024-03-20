#![feature(strict_provenance)]
#![feature(inline_const)]
#![no_std]
#![no_main]

use core::{arch::asm, ops::RangeToInclusive};
use cortex_m::asm::nop;
use cortex_m_rt::entry;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

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
impl From<PortError> for WriteError {
    #[inline]
    fn from(value: PortError) -> Self {
        WriteError(value.0)
    }
}
pub trait Port {
    type Kind: PortKind;
    const BASE: usize;
    const RANGE: RangeToInclusive<u8>;
    #[inline]
    fn is_valid(pin_number: u8) -> Result<(), PortError> {
        if pin_number < Self::RANGE.end {
            Ok(())
        } else {
            Err(PortError(ErrorKind::BadIndex))
        }
    }
}

pub trait PortOffset {
    const OFFSET: usize;
}

pub trait GPIOPort {
    const BASE: usize;
    const RANGE: RangeToInclusive<u8>;
}

impl<GP: GPIOPort> Port for GP {
    type Kind = GPIO;
    const BASE: usize = <Self as GPIOPort>::BASE;
    const RANGE: RangeToInclusive<u8> = <Self as GPIOPort>::RANGE;
}

pub enum Access {
    R,
    W,
    RW,
}

pub trait Register<P: Port, O: PortOffset> {
    const REG_ADDR: usize = P::BASE + O::OFFSET;
    const ACCESS: Access;
    fn ptr() -> *mut usize {
        core::ptr::null_mut::<usize>().with_addr(Self::REG_ADDR)
    }
    fn ptr_from(pin_id: u32) -> *mut usize {
        core::ptr::null_mut::<usize>().with_addr(Self::REG_ADDR | pin_id as usize)
    }
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
pub trait WriteRegister<P: Port, O: PortOffset>: Register<P, O> {
    #[inline]
    fn write(pin_number: u8, value: Pinstate) -> Result<(), WriteError> {
        if pin_number > P::RANGE.end {
            return Err(WriteError(ErrorKind::BadIndex));
        };

        let pin_id = 1 << (pin_number as usize);

        let address = (Self::REG_ADDR as usize) as *mut usize;

        let register_value = unsafe { core::ptr::read_volatile(address) };

        let masked_value = register_value & pin_id;

        let is_high = masked_value != 1;

        // If the bits mismatch
        if is_high ^ value.is_high() {
            // Flip only that bit
            let value = (pin_id as usize) ^ register_value;
            rprintln!(
                "{:#x}: {:#x} => {:#x}",
                address.addr(),
                register_value,
                value
            );
            unsafe { core::ptr::write_volatile(address, value as usize) };
        } else {
        }

        // rprintln!("{:#x}", address.addr());

        // unsafe {
        //     asm!(
        //         "ldr r0, [{},#0]",
        //         in(reg) core::mem::transmute::<Pinstate, u32>(value),
        //         in("r0") (Self::REG_ADDR + pin_id as usize),
        //     )
        // };

        Ok(())
    }
}

pub struct ReadError(ErrorKind);
impl core::fmt::Debug for ReadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("Read error: {:?}", self.0))
    }
}

pub trait ReadRegister<P: Port, O: PortOffset>: Register<P, O> {
    #[inline]
    fn read(pin_number: u8) -> Result<Pinstate, ReadError> {
        P::is_valid(pin_number)?;

        let pin_id = 1 << (pin_number as usize);

        let address = (Self::REG_ADDR as usize) as *mut usize;
        // let mut out: u32;
        let register_value = unsafe { core::ptr::read_volatile(address) };
        let masked_value = register_value & pin_id;
        rprintln!("{:#x}: {:#x}", address.addr(), masked_value);
        Ok((masked_value as u32).into())
        // let current_value =
        //     (unsafe { core::ptr::read_volatile(address) } & pin_id >> pin_id as usize) as u32;

        // rprintln!("{:#x}: {:#x}", address.addr(), current_value);
        // Ok(current_value.into())
        // Ok(if (current_value >> pin_number as usize) == 1 {
        //     Pinstate::High
        // } else {
        //     Pinstate::Low
        // })

        // unsafe {
        //     // asm!(
        //     //     "mov {}, r0",
        //     //     out(reg) out,
        //     //     in("r0") (Self::REG_ADDR + pin_id as usize),
        //     // );
        //     let out = core::ptr::read_volatile(address) as u32;

        //     Ok(out.into())
        // }
    }
}

pub trait RegisterArray<P: Port, O: PortOffset, const COUNT: usize> {
    const REG_ADDRS: [usize; COUNT] = const {
        let mut idx = 0;
        let mut addrs = [P::BASE + O::OFFSET; COUNT];
        while idx < COUNT {
            addrs[idx] += idx * 0x4;
            idx += 1;
        }
        addrs
    };
    const ACCESS: Access;
}

pub trait WriteRegisterArray<P: Port, O: PortOffset, const COUNT: usize>:
    RegisterArray<P, O, COUNT>
{
    #[inline]
    fn write_array(pin_number: u8, value: u32) -> Result<(), WriteError> {
        P::is_valid(pin_number)?;

        // Bits 17 and 16 => SENSE
        // Bits 10, 9, and 8 => DRIVE
        // Bits 3 and 2 => PULL
        // Bit 1 => INPUT
        // Bit 0 => DIR
        let pin_id = 1usize << (pin_number as usize);

        let address = Self::REG_ADDRS[pin_id] as *mut usize;
        rprintln!("{:#x}", address.addr());

        unsafe { core::ptr::write_volatile(address, value as usize) };

        // unsafe {
        //     asm!("ldr r0, [{},#0]",
        //         in(reg) value,
        //         in("r0") address
        //     );
        // }

        Ok(())
    }
}
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
mod registers {
    use super::GPIOPort;
    use core::{arch::asm, ops::RangeToInclusive};
    /// General purpose input and output port
    /// P0.00 to P0.31 implemented
    pub struct P0;
    impl GPIOPort for P0 {
        const BASE: usize = 0x5000_0000;
        const RANGE: RangeToInclusive<u8> = ..=31;
    }
    /// General purpose input and output port
    /// P1.00 to P1.09 implemented
    pub struct P1;
    impl GPIOPort for P1 {
        const BASE: usize = 0x5000_0300;
        const RANGE: RangeToInclusive<u8> = ..=9;
    }
    macro_rules! def_portoffset {
        ($name:ident, $addr:literal, $comment:literal) => {
            #[doc = $comment]
            pub struct $name;
            impl crate::PortOffset for $name {
                const OFFSET: usize = $addr;
            }
        };
    }
    macro_rules! reg {
        ($name:ident, $addr:literal, RW, $comment:literal) => {
            def_portoffset!($name, $addr, $comment);
            paste::paste! {
                #[doc = $comment "bank" 0]
                pub struct [<$name 0>];
                impl crate::Register<P0, $name> for [<$name 0>] {
                    const ACCESS: crate::Access = RW;
                }
                impl crate::ReadRegister<P0, $name> for [<$name 0>] {}
                impl crate::WriteRegister<P0, $name> for [<$name 0>] {}
                #[doc = $comment "bank" 1]
                pub struct [<$name 1>];
                impl crate::Register<P1, $name> for [<$name 1>] {
                    const ACCESS: crate::Access = RW;
                }
                impl crate::ReadRegister<P1, $name> for [<$name 1>] {}
                impl crate::WriteRegister<P1, $name> for [<$name 1>] {}
            }
        };
        ($name:ident, $addr:literal, R, $comment:literal) => {
            def_portoffset!($name, $addr, $comment);
            paste::paste! {
                #[doc = $comment "bank" 0]
                pub struct [<$name 0>];
                impl crate::Register<P0, $name> for [<$name 0>] {
                    const ACCESS: crate::Access = RW;
                }
                impl crate::ReadRegister<P0, $name> for [<$name 0>] {}
                #[doc = $comment "bank" 1]
                pub struct [<$name 1>];
                impl crate::Register<P1, $name> for [<$name 1>] {
                    const ACCESS: crate::Access = RW;
                }
                impl crate::ReadRegister<P1, $name> for [<$name 1>] {}
            }
        };
        ($name:ident, $addr:literal, $acc:ident, $comment:literal) => {
            def_portoffset!($name, $addr, $comment);
            paste::paste! {
                #[doc = $comment "bank" 0]
                pub struct [<$name 0>];
                impl crate::Register<P0, $name> for [<$name 0>] {
                    const ACCESS: crate::Access = $acc;
                }
                #[doc = $comment "bank" 1]
                pub struct [<$name 1>];
                impl crate::Register<P1, $name> for [<$name 1>] {
                    const ACCESS: crate::Access = $acc;
                }
                impl [<$name REG>]<P1, $name> for [<$name 1>] {}
            }
        };
    }
    macro_rules! rar {
        ($name:ident, $addr:literal, $count:literal, RW, $comment:literal) => {
            def_portoffset!($name, $addr, $comment);
            paste::paste! {
                #[doc = $comment "bank" 0]
                pub struct [<$name 0>];
                impl crate::RegisterArray<P0, $name, $count> for [<$name 0>] {
                    const ACCESS: crate::Access = RW;
                }
                // impl crate::ReadRegisterArray<P0, $name, $count> for [<$name 0>] {}
                impl crate::WriteRegisterArray<P0, $name, $count> for [<$name 0>] {}
                #[doc = $comment 1]
                pub struct [<$name 1>];
                impl crate::RegisterArray<P1, $name, $count> for [<$name 1>] {
                    const ACCESS: crate::Access = RW;
                }
                // impl crate::ReadRegisterArray<P1, $name, $count> for [<$name 1>] {}
                impl crate::WriteRegisterArray<P1, $name, $count> for [<$name 1>] {}
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
    impl core::ops::Index<u32> for PINCNF0 {
        type Output = usize;
        fn index(&self, index: u32) -> &Self::Output {
            &Self::REG_ADDRS[index as usize]
        }
    }
    impl core::ops::BitOrAssign<(u8, u32)> for PINCNF0 {
        fn bitor_assign(&mut self, rhs: (u8, u32)) {
            Self::write_array(rhs.0, rhs.1).unwrap()
        }
    }
    use crate::{Access::*, Pinstate, RegisterArray, WriteRegisterArray};
    reg!(OUT, 0x504, RW, "Write GPIO port");
    pub trait OUTREG<P: crate::Port, O: crate::PortOffset>: crate::Register<P, O> {
        fn write(value: Pinstate) {
            let intermediate: u32 = value.into();
            unsafe {
                asm!(
                    "ldr r0, {0}",
                    in(reg) intermediate,
                    in("r0") Self::REG_ADDR
                );
            }
        }
    }
    reg!(OUTSET, 0x508, RW, "Set individual bits in GPIO port");
    pub trait OUTSETREG<P: crate::Port, O: crate::PortOffset>: crate::Register<P, O> {}
    reg!(OUTCLR, 0x50C, RW, "Clear individual bits in GPIO port");
    pub trait OUTCLRREG<P: crate::Port, O: crate::PortOffset>: crate::Register<P, O> {}
    reg!(IN, 0x510, R, "Read GPIO port");
    pub trait INREG<P: crate::Port, O: crate::PortOffset>: crate::Register<P, O> {
        fn read() -> Pinstate {
            let mut value: u32;
            unsafe {
                asm!(
                    "mov {}, r0",
                    out(reg) value,
                    in("r0") Self::REG_ADDR
                );
            }
            value.into()
        }
    }
    reg!(DIR, 0x514, RW, "Direction of GPIO pins");
    reg!(DIRSET, 0x518, RW, "Set direction of GPIO pins");
    reg!(DIRCLR, 0x51C, RW, "Clear direction of GPIO pins");
    reg!(LATCH, 0x520, RW, "Latch register indicating what GPIO pins that have met the criteria set in the [PIN_CNF[n]]. SENSE registers");
    reg!(
        DETECTMODE,
        0x524,
        RW,
        "Select between default DETECT signal behavior and LDETECT mode"
    );
    rar!(PINCNF, 0x700, 32, RW, "Configuration of GPIO pins");
}
use registers::{DIR0, IN0, OUT0, P0, P1, PINCNF0, PINCNF1};
#[entry]
fn main() -> ! {
    rtt_init_print!();
    rprintln!("IM WOKE");
    // Connect input buffer and set as input, pulldown
    let _ = PINCNF0::write_array(1, 0b0000);
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

    let value = IN0::read(1).unwrap();
    rprintln!("pinstate {}", value);
    // Configure as output
    // let _ = PINCNF0::write_array(1, 0x0001).unwrap();
    // let _ = OUT0::write(1, Pinstate::Low).unwrap();
    let value = IN0::read(1).unwrap();
    // rprintln!("pinstate {}", value);
    loop {
        // rprintln!("Soulja boy tell em");
        rprintln!("Pulling up");
        let _ = PINCNF0::write_array(1, 0b1100).unwrap();
        for _ in 0..1_000_000 {
            nop();
        }
        let value = IN0::read(1).unwrap();
        rprintln!("pinstate {}", value);
        rprintln!("Pulling down");
        let _ = PINCNF0::write_array(1, 0b0100);
        // let _ = PINCNF0::write_array(1, 0b0100).unwrap();
        for _ in 0..1_000_000 {
            nop();
        }
    }
}