use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

pub trait GpioExt {
    type Parts;

    fn split(self) -> Self::Parts;
}

pub struct Locked;

pub struct Unlocked;

pub struct Analog;

pub struct Input<MODE> {
    _typestate_mode: PhantomData<MODE>,
}

pub struct Floating;

pub struct PullDown;

pub struct PullUp;

pub struct Output<MODE, SPEED> {
    _typestate_mode: PhantomData<MODE>,
    _typestate_speed: PhantomData<SPEED>,
}

pub struct Alternate<MODE, SPEED> {
    _typestate_mode: PhantomData<MODE>,
    _typestate_speed: PhantomData<SPEED>,
}

pub struct PushPull;

pub struct OpenDrain;

pub trait InputMode {}

impl InputMode for Floating {}

impl InputMode for PullDown {}

impl InputMode for PullUp {}

pub trait OutputMode {}

impl OutputMode for PushPull {}

impl OutputMode for OpenDrain {}

pub trait AlternateMode {}

impl AlternateMode for PushPull {}

impl AlternateMode for OpenDrain {}

pub trait Active {}

impl Active for Analog {}

impl<MODE> Active for Input<MODE> where MODE: InputMode {}

impl<MODE, SPEED> Active for Output<MODE, SPEED>
where
    MODE: OutputMode,
    SPEED: Speed,
{
}

impl<MODE, SPEED> Active for Alternate<MODE, SPEED>
where
    MODE: AlternateMode,
    SPEED: Speed,
{
}

pub struct UpTo10MHz;

pub struct UpTo2MHz;

pub struct UpTo50MHz;

pub trait Speed {
    const MD_BITS: u32;
}

impl Speed for UpTo50MHz {
    const MD_BITS: u32 = 0b11;
}

impl Speed for UpTo10MHz {
    const MD_BITS: u32 = 0b01;
}

impl Speed for UpTo2MHz {
    const MD_BITS: u32 = 0b10;
}

#[inline]
fn atomic_set_bit(r: &AtomicU32, is_one: bool, index: usize) {
    let mask = 1 << index;
    match is_one {
        true => r.fetch_or(mask, Ordering::SeqCst),
        false => r.fetch_nand(mask, Ordering::SeqCst),
    };
}

#[inline]
fn atomic_toggle_bit(r: &AtomicU32, index: usize) {
    let mask = 1 << index;
    r.fetch_xor(mask, Ordering::SeqCst);
}

trait PinIndex {
    const CTL_MD_INDEX: usize;

    const OP_LK_INDEX: usize;
}

pub mod gpioa {
    use super::{
        Active, Alternate, AlternateMode, Analog, Floating, GpioExt, Input, InputMode, Locked,
        OpenDrain, Output, OutputMode, PinIndex, PullDown, PullUp, PushPull, Speed, Unlocked,
    };
    use crate::pac::{gpioa, GPIOA};
    use core::convert::Infallible;
    use core::marker::PhantomData;
    use core::sync::atomic::AtomicU32;
    use embedded_hal::digital::v2::{InputPin, OutputPin, StatefulOutputPin, ToggleableOutputPin};

    pub struct Parts {
        pub ctl0: CTL0,
        //ctl1
        pub octl: OCTL,
        pub lock: LOCK, // todo: port-A global lock typestate machine
        pub pa0: PA0<Unlocked, Input<Floating>>,
        //pa1, ..
    }

    impl GpioExt for GPIOA {
        type Parts = Parts;

        fn split(self) -> Self::Parts {
            Parts {
                ctl0: CTL0 { _ownership: () },
                // ...
                octl: OCTL { _ownership: () },
                lock: LOCK { _ownership: () },
                pa0: PA0 {
                    _typestate_locked: PhantomData,
                    _typestate_mode: PhantomData,
                },
                // ...
            }
        }
    }

    pub struct CTL0 {
        _ownership: (),
    }

    impl CTL0 {
        pub(crate) fn ctl0(&mut self) -> &gpioa::CTL0 {
            unsafe { &(*GPIOA::ptr()).ctl0 }
        }
    }

    pub struct OCTL {
        _ownership: (),
    }

    impl OCTL {
        pub(crate) fn octl(&mut self) -> &gpioa::OCTL {
            unsafe { &(*GPIOA::ptr()).octl }
        }
    }

    pub struct LOCK {
        _ownership: (),
    }

    impl LOCK {
        pub(crate) fn lock(&mut self) -> &gpioa::LOCK {
            unsafe { &(*GPIOA::ptr()).lock }
        }
        
        // todo: change a name?
        pub fn lock_port(mut self) -> Result<(), LOCK> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(self.lock()) };
            super::atomic_set_bit(r, true, 16);
            super::atomic_set_bit(r, false, 16);
            super::atomic_set_bit(r, true, 16);
            let ans1 = self.lock().read().bits() & (1 << 16);
            let ans2 = self.lock().read().bits() & (1 << 16);
            if ans1 == 0 && ans2 == 1 {
                Ok(())
            } else {
                Err(self)
            }
        }
    }

    pub struct PA0<LOCKED, MODE> {
        _typestate_locked: PhantomData<LOCKED>,
        _typestate_mode: PhantomData<MODE>,
    }

    impl<LOCKED, MODE> PinIndex for PA0<LOCKED, MODE> {
        const CTL_MD_INDEX: usize = 0;

        const OP_LK_INDEX: usize = 0;
    }

    impl<MODE> PA0<Unlocked, MODE>
    where
        MODE: Active,
    {
        pub fn into_analog(self, ctl0: &mut CTL0) -> PA0<Unlocked, Analog> {
            self.into_with_ctrl_md(ctl0, 0b00_00)
        }

        pub fn into_floating_input(self, ctl0: &mut CTL0) -> PA0<Unlocked, Input<Floating>> {
            self.into_with_ctrl_md(ctl0, 0b01_00)
        }

        pub fn into_pull_down_input(
            self,
            ctl0: &mut CTL0,
            octl: &mut OCTL,
        ) -> PA0<Unlocked, Input<PullDown>> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(octl.octl()) };
            super::atomic_set_bit(r, false, Self::OP_LK_INDEX);
            self.into_with_ctrl_md(ctl0, 0b10_00)
        }

        pub fn into_pull_up_input(
            self,
            ctl0: &mut CTL0,
            octl: &mut OCTL,
        ) -> PA0<Unlocked, Input<PullUp>> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(octl.octl()) };
            super::atomic_set_bit(r, true, Self::OP_LK_INDEX);
            self.into_with_ctrl_md(ctl0, 0b10_00)
        }

        pub fn into_push_pull_output_speed<SPEED: Speed>(
            self,
            ctl0: &mut CTL0,
        ) -> PA0<Unlocked, Output<PushPull, SPEED>> {
            let ctrl_md = 0b00_00 | SPEED::MD_BITS;
            self.into_with_ctrl_md(ctl0, ctrl_md)
        }

        pub fn into_open_drain_output_speed<SPEED: Speed>(
            self,
            ctl0: &mut CTL0,
        ) -> PA0<Unlocked, Output<OpenDrain, SPEED>> {
            let ctrl_md = 0b01_00 | SPEED::MD_BITS;
            self.into_with_ctrl_md(ctl0, ctrl_md)
        }

        pub fn into_push_pull_alternate_speed<SPEED: Speed>(
            self,
            ctl0: &mut CTL0,
        ) -> PA0<Unlocked, Alternate<PushPull, SPEED>> {
            let ctrl_md = 0b10_00 | SPEED::MD_BITS;
            self.into_with_ctrl_md(ctl0, ctrl_md)
        }

        pub fn into_open_drain_alternate_speed<SPEED: Speed>(
            self,
            ctl0: &mut CTL0,
        ) -> PA0<Unlocked, Alternate<OpenDrain, SPEED>> {
            let ctrl_md = 0b11_00 | SPEED::MD_BITS;
            self.into_with_ctrl_md(ctl0, ctrl_md)
        }

        #[inline]
        fn into_with_ctrl_md<T>(self, ctl0: &mut CTL0, ctl_and_md: u32) -> PA0<Unlocked, T> {
            ctl0.ctl0().modify(|r, w| unsafe {
                w.bits(
                    (r.bits() & !(0b1111 << Self::CTL_MD_INDEX))
                        | (ctl_and_md << Self::CTL_MD_INDEX),
                )
            });
            PA0 {
                _typestate_locked: PhantomData,
                _typestate_mode: PhantomData,
            }
        }

        pub fn lock(self, lock: &mut LOCK) -> PA0<Locked, MODE> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(lock.lock()) };
            super::atomic_set_bit(r, true, Self::OP_LK_INDEX);
            PA0 {
                _typestate_locked: PhantomData,
                _typestate_mode: PhantomData,
            }
        }
    }

    impl<MODE> PA0<Locked, MODE>
    where
        MODE: Active,
    {
        pub fn unlock(self, lock: &mut LOCK) -> PA0<Unlocked, MODE> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(lock.lock()) };
            super::atomic_set_bit(r, false, Self::OP_LK_INDEX);
            PA0 {
                _typestate_locked: PhantomData,
                _typestate_mode: PhantomData,
            }
        }
    }

    impl<MODE, SPEED> PA0<Unlocked, Output<MODE, SPEED>>
    where
        MODE: OutputMode,
        SPEED: Speed,
    {
        pub fn into_push_pull_output(
            self,
            ctl0: &mut CTL0,
        ) -> PA0<Unlocked, Output<PushPull, SPEED>> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(ctl0.ctl0()) };
            super::atomic_set_bit(r, false, Self::CTL_MD_INDEX);
            PA0 {
                _typestate_locked: PhantomData,
                _typestate_mode: PhantomData,
            }
        }

        pub fn into_open_drain_output(
            self,
            ctl0: &mut CTL0,
        ) -> PA0<Unlocked, Output<OpenDrain, SPEED>> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(ctl0.ctl0()) };
            super::atomic_set_bit(r, true, Self::CTL_MD_INDEX);
            PA0 {
                _typestate_locked: PhantomData,
                _typestate_mode: PhantomData,
            }
        }
    }

    impl<MODE, SPEED> PA0<Unlocked, Alternate<MODE, SPEED>>
    where
        MODE: AlternateMode,
        SPEED: Speed,
    {
        pub fn into_push_pull_alternate(
            self,
            ctl0: &mut CTL0,
        ) -> PA0<Unlocked, Alternate<PushPull, SPEED>> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(ctl0.ctl0()) };
            super::atomic_set_bit(r, false, Self::CTL_MD_INDEX);
            PA0 {
                _typestate_locked: PhantomData,
                _typestate_mode: PhantomData,
            }
        }

        pub fn into_open_drain_alternate(
            self,
            ctl0: &mut CTL0,
        ) -> PA0<Unlocked, Alternate<OpenDrain, SPEED>> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(ctl0.ctl0()) };
            super::atomic_set_bit(r, true, Self::CTL_MD_INDEX);
            PA0 {
                _typestate_locked: PhantomData,
                _typestate_mode: PhantomData,
            }
        }
    }

    impl<LOCKED, MODE> InputPin for PA0<LOCKED, Input<MODE>>
    where
        MODE: InputMode,
    {
        type Error = Infallible;

        fn is_high(&self) -> Result<bool, Self::Error> {
            let ans =
                (unsafe { &(*GPIOA::ptr()).istat }.read().bits() & (1 << Self::OP_LK_INDEX)) != 0;
            Ok(ans)
        }

        fn is_low(&self) -> Result<bool, Self::Error> {
            Ok(!self.is_high()?)
        }
    }

    impl<LOCKED, MODE, SPEED> OutputPin for PA0<LOCKED, Output<MODE, SPEED>>
    where
        MODE: OutputMode,
        SPEED: Speed,
    {
        type Error = Infallible;

        fn set_high(&mut self) -> Result<(), Self::Error> {
            unsafe { &(*GPIOA::ptr()).bop }.write(|w| unsafe { w.bits(1 << Self::OP_LK_INDEX) });
            Ok(())
        }

        fn set_low(&mut self) -> Result<(), Self::Error> {
            unsafe { &(*GPIOA::ptr()).bc }.write(|w| unsafe { w.bits(1 << Self::OP_LK_INDEX) });
            Ok(())
        }
    }

    impl<LOCKED, MODE, SPEED> OutputPin for PA0<LOCKED, Alternate<MODE, SPEED>>
    where
        MODE: AlternateMode,
        SPEED: Speed,
    {
        type Error = Infallible;

        fn set_high(&mut self) -> Result<(), Self::Error> {
            unsafe { &(*GPIOA::ptr()).bop }.write(|w| unsafe { w.bits(1 << Self::OP_LK_INDEX) });
            Ok(())
        }

        fn set_low(&mut self) -> Result<(), Self::Error> {
            unsafe { &(*GPIOA::ptr()).bc }.write(|w| unsafe { w.bits(1 << Self::OP_LK_INDEX) });
            Ok(())
        }
    }

    impl<LOCKED, MODE, SPEED> StatefulOutputPin for PA0<LOCKED, Output<MODE, SPEED>>
    where
        MODE: OutputMode,
        SPEED: Speed,
    {
        fn is_set_high(&self) -> Result<bool, Self::Error> {
            let ans =
                (unsafe { &(*GPIOA::ptr()).octl }.read().bits() & (1 << Self::OP_LK_INDEX)) != 0;
            Ok(ans)
        }

        fn is_set_low(&self) -> Result<bool, Self::Error> {
            Ok(!self.is_set_high()?)
        }
    }

    impl<LOCKED, MODE, SPEED> StatefulOutputPin for PA0<LOCKED, Alternate<MODE, SPEED>>
    where
        MODE: AlternateMode,
        SPEED: Speed,
    {
        fn is_set_high(&self) -> Result<bool, Self::Error> {
            let ans =
                (unsafe { &(*GPIOA::ptr()).octl }.read().bits() & (1 << Self::OP_LK_INDEX)) != 0;
            Ok(ans)
        }

        fn is_set_low(&self) -> Result<bool, Self::Error> {
            Ok(!self.is_set_high()?)
        }
    }

    impl<LOCKED, MODE, SPEED> ToggleableOutputPin for PA0<LOCKED, Output<MODE, SPEED>>
    where
        MODE: OutputMode,
        SPEED: Speed,
    {
        type Error = Infallible;

        fn toggle(&mut self) -> Result<(), Self::Error> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(&(*GPIOA::ptr()).octl) };
            super::atomic_toggle_bit(r, Self::OP_LK_INDEX);
            Ok(())
        }
    }

    impl<LOCKED, MODE, SPEED> ToggleableOutputPin for PA0<LOCKED, Alternate<MODE, SPEED>>
    where
        MODE: AlternateMode,
        SPEED: Speed,
    {
        type Error = Infallible;

        fn toggle(&mut self) -> Result<(), Self::Error> {
            let r: &AtomicU32 = unsafe { core::mem::transmute(&(*GPIOA::ptr()).octl) };
            super::atomic_toggle_bit(r, Self::OP_LK_INDEX);
            Ok(())
        }
    }

    impl<LOCKED, SPEED> InputPin for PA0<LOCKED, Output<OpenDrain, SPEED>>
    where
        SPEED: Speed,
    {
        type Error = Infallible;

        fn is_high(&self) -> Result<bool, Self::Error> {
            let ans =
                (unsafe { &(*GPIOA::ptr()).istat }.read().bits() & (1 << Self::OP_LK_INDEX)) != 0;
            Ok(ans)
        }

        fn is_low(&self) -> Result<bool, Self::Error> {
            Ok(!self.is_high()?)
        }
    }

}
